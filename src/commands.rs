//! Command implementations for kotlin-index CLI
//!
//! This module contains all command implementations:
//! - grep: Search commands (grep, find_class, find_file, etc.)
//! - management: Index management (rebuild, stats)
//! - index: File indexing operations
//! - modules: Module-related commands
//! - files: File operations (outline, stats)
//! - android: Android-specific (resources, strings)
//! - ios: iOS-specific commands
//! - perl: Perl-specific commands

pub mod analysis;
pub mod android;
pub mod common;
pub mod files;
pub mod grep;
pub mod index;
pub mod ios;
pub mod management;
pub mod modules;
pub mod perl;
pub mod project_info;
pub mod watch;

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::{Context, Result};
use crossbeam_channel as channel;
use grep_regex::RegexMatcher;
use grep_searcher::MmapChoice;
use grep_searcher::{SearcherBuilder, sinks::UTF8};
use ignore::WalkBuilder;

use crate::db;

/// Check if no_ignore mode is enabled for this project
pub fn is_no_ignore_enabled(root: &Path) -> bool {
    if let Ok(conn) = db::open_db(root) {
        let result: Result<String, _> = conn.query_row(
            "SELECT value FROM metadata WHERE key = 'no_ignore'",
            [],
            |row| row.get(0),
        );
        return result.map(|v| v == "1").unwrap_or(false);
    }
    false
}

/// Get number of available CPU cores
pub fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

/// Get relative path from root
pub fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

/// Fast parallel file search using grep-searcher and ignore crates.
/// Delegates to `search_files_limited` with no effective limit.
pub fn search_files<F>(root: &Path, pattern: &str, extensions: &[&str], handler: F) -> Result<()>
where
    F: FnMut(&Path, usize, &str),
{
    search_files_limited(root, pattern, extensions, usize::MAX, handler)
}

/// Fast parallel file search with early termination support
pub fn search_files_limited<F>(
    root: &Path,
    pattern: &str,
    extensions: &[&str],
    limit: usize,
    mut handler: F,
) -> Result<()>
where
    F: FnMut(&Path, usize, &str),
{
    let matcher = RegexMatcher::new(pattern).context("Invalid regex pattern")?;
    let no_ignore = is_no_ignore_enabled(root);
    let use_git = crate::indexer::has_git_repo(root) && !no_ignore;
    let arc_root = if no_ignore {
        None
    } else {
        crate::indexer::find_arc_root(root)
    };

    let mut wb = WalkBuilder::new(root);
    wb.hidden(true)
        .git_ignore(use_git)
        .git_exclude(use_git)
        .filter_entry(|entry| !crate::indexer::is_excluded_dir(entry))
        .threads(num_cpus());
    crate::indexer::configure_walk_ignores(&mut wb, arc_root.as_deref());
    let walker = wb.build_parallel();

    let (tx, rx) = channel::bounded::<(Arc<Path>, usize, String)>(limit.clamp(1_000, 10_000));

    let extensions: Arc<HashSet<String>> =
        Arc::new(extensions.iter().map(|s| s.to_string()).collect());

    // Shared counter for early termination
    let found_count = Arc::new(AtomicUsize::new(0));
    let should_stop = Arc::new(AtomicBool::new(false));

    walker.run(|| {
        let tx = tx.clone();
        let matcher = matcher.clone();
        let extensions = Arc::clone(&extensions);
        let found_count = Arc::clone(&found_count);
        let should_stop = Arc::clone(&should_stop);

        // SAFETY: memory-mapped files are safe when files aren't modified during search
        let mut searcher = SearcherBuilder::new()
            .memory_map(unsafe { MmapChoice::auto() })
            .line_number(true)
            .build();

        Box::new(move |entry| {
            // Check early termination
            if should_stop.load(Ordering::Relaxed) {
                return ignore::WalkState::Quit;
            }

            if let Ok(entry) = entry {
                let path = entry.path();
                if let Some(ext) = path.extension()
                    && extensions.contains(ext.to_str().unwrap_or(""))
                {
                    let path_arc: Arc<Path> = Arc::from(path);
                    let found_count = Arc::clone(&found_count);
                    let should_stop = Arc::clone(&should_stop);

                    let _ = searcher.search_path(
                        &matcher,
                        path,
                        UTF8(|line_num, line| {
                            // Check if we should stop
                            if should_stop.load(Ordering::Relaxed) {
                                return Ok(false); // Stop searching this file
                            }

                            let count = found_count.fetch_add(1, Ordering::Relaxed);
                            if count >= limit {
                                should_stop.store(true, Ordering::Relaxed);
                                return Ok(false);
                            }

                            let _ = tx.send((
                                Arc::clone(&path_arc),
                                line_num as usize,
                                line.trim_end().to_string(),
                            ));
                            Ok(true)
                        }),
                    );
                }
            }
            ignore::WalkState::Continue
        })
    });

    drop(tx);

    for (count, (path, line_num, line)) in rx.into_iter().enumerate() {
        if count >= limit {
            break;
        }
        handler(&path, line_num, &line);
    }

    Ok(())
}
