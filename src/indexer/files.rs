//! File indexing: directory walk, parallel parsing, and DB write operations.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::Result;
use rayon::prelude::*;
use rusqlite::Connection;

use crate::parsers;

use super::{
    INCREMENTAL_PROGRESS_INTERVAL, MAX_FILE_SIZE, MAX_WALK_DEPTH, PARSE_CHUNK_SIZE,
    PARSE_PROGRESS_INTERVAL, ParsedFile, WALK_PROGRESS_INTERVAL, WalkResult, build_thread_pool,
    configure_walk_ignores, detect_project_type, find_arc_root, has_git_repo, is_excluded_dir,
    is_module_file,
};

pub(crate) fn parse_file(root: &Path, file_path: &Path) -> Result<ParsedFile> {
    let metadata = fs::metadata(file_path)?;
    let mtime = metadata
        .modified()?
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs() as i64;
    let size = metadata.len() as i64;

    let rel_path = file_path
        .strip_prefix(root)
        .unwrap_or(file_path)
        .to_string_lossy()
        .to_string();

    // Skip files larger than MAX_FILE_SIZE (likely generated/minified)
    if size > MAX_FILE_SIZE as i64 {
        return Ok(ParsedFile {
            rel_path,
            mtime,
            size,
            symbols: vec![],
            refs: vec![],
        });
    }

    let content = fs::read_to_string(file_path)?;

    // Detect file type by extension
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let file_type = match parsers::FileType::from_extension(ext) {
        Some(ft) => ft,
        None => {
            return Ok(ParsedFile {
                rel_path,
                mtime,
                size,
                symbols: vec![],
                refs: vec![],
            });
        }
    };

    let (symbols, refs) = parsers::parse_file_symbols(&content, file_type)?;

    Ok(ParsedFile {
        rel_path,
        mtime,
        size,
        symbols,
        refs,
    })
}

pub fn index_directory(
    conn: &mut Connection,
    root: &Path,
    progress: bool,
    no_ignore: bool,
) -> Result<WalkResult> {
    index_directory_scoped(conn, root, root, progress, no_ignore)
}

pub fn index_directory_scoped(
    conn: &mut Connection,
    root: &Path,
    walk_dir: &Path,
    progress: bool,
    no_ignore: bool,
) -> Result<WalkResult> {
    use ignore::WalkBuilder;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    let verbose = std::env::var("AST_INDEX_VERBOSE").is_ok();

    // Small chunks: parse PARSE_CHUNK_SIZE files in parallel → write to DB → free memory → next chunk
    // Peak memory: ~PARSE_CHUNK_SIZE × (file content + ParsedFile), then freed each iteration

    // Detect project type
    let project_type = detect_project_type(walk_dir);
    if progress {
        eprintln!("Detected project type: {}", project_type.as_str());
    }

    // Collect all file paths (paths are lightweight, OK to keep in memory)
    if verbose {
        eprintln!(
            "[verbose] checking git repo: walk_dir={}",
            walk_dir.display()
        );
    }
    let t = Instant::now();
    let use_git = has_git_repo(walk_dir) || has_git_repo(root);
    let use_git = use_git && !no_ignore;
    if verbose {
        eprintln!("[verbose] has_git_repo: {} in {:?}", use_git, t.elapsed());
    }

    let t = Instant::now();
    let arc_root = if no_ignore {
        None
    } else {
        find_arc_root(walk_dir).or_else(|| find_arc_root(root))
    };
    if verbose {
        eprintln!(
            "[verbose] find_arc_root: {:?} in {:?}",
            arc_root.as_ref().map(|p| p.display().to_string()),
            t.elapsed()
        );
    }

    let mut builder = WalkBuilder::new(walk_dir);
    builder
        .hidden(true)
        .follow_links(false) // Never follow symlinks — prevents loops in monorepos
        .max_depth(Some(MAX_WALK_DEPTH)) // Prevent runaway traversal in deeply nested structures
        .git_ignore(use_git) // Respect .gitignore only if .git exists
        .git_exclude(use_git)
        .filter_entry(|entry| !is_excluded_dir(entry));
    // Arc repos: respect .gitignore and .arcignore without .git directory
    if verbose && arc_root.is_some() {
        eprintln!("[verbose] arc mode: adding .gitignore + .arcignore custom ignore filenames");
        if let Some(ref arc) = arc_root {
            let root_gitignore = arc.join(".gitignore");
            if root_gitignore.exists() {
                eprintln!(
                    "[verbose] adding root .gitignore: {}",
                    root_gitignore.display()
                );
            }
        }
    }
    configure_walk_ignores(&mut builder, arc_root.as_deref());

    if verbose {
        eprintln!("[verbose] starting file walk...");
    }
    let walk_start = Instant::now();
    let walker = builder.build();

    let mut files: Vec<PathBuf> = Vec::new();
    let mut module_files: Vec<PathBuf> = Vec::new();
    let mut storyboard_files: Vec<PathBuf> = Vec::new();
    let mut xcassets_dirs: Vec<PathBuf> = Vec::new();
    let mut xml_layout_files: Vec<PathBuf> = Vec::new();
    let mut res_files: Vec<PathBuf> = Vec::new();

    let mut walk_entries = 0usize;
    for entry in walker.filter_map(|e| e.ok()) {
        walk_entries += 1;
        if verbose && walk_entries.is_multiple_of(WALK_PROGRESS_INTERVAL) {
            eprintln!(
                "[verbose] walk: {} entries scanned in {:?}...",
                walk_entries,
                walk_start.elapsed()
            );
        }
        let path = entry.path();
        // Collect module-related files for index_modules
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && is_module_file(name)
        {
            module_files.push(path.to_path_buf());
        }
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            // Collect parseable source files
            if parsers::is_supported_extension(ext) {
                files.push(path.to_path_buf());
            }
            // Collect storyboard/xib files (iOS)
            if ext == "storyboard" || ext == "xib" {
                storyboard_files.push(path.to_path_buf());
            }
            // Collect .xcassets directories (iOS)
            if ext == "xcassets" && path.is_dir() {
                xcassets_dirs.push(path.to_path_buf());
            }
            // Collect Android resource files
            let path_str = path.to_string_lossy();
            if path_str.contains("/res/") {
                res_files.push(path.to_path_buf());
                // XML layout/menu/navigation files
                if ext == "xml"
                    && (path_str.contains("/layout")
                        || path_str.contains("/menu")
                        || path_str.contains("/navigation"))
                {
                    xml_layout_files.push(path.to_path_buf());
                }
            }
        }
    }

    if verbose {
        eprintln!(
            "[verbose] walk complete: {} total entries, {} source files, {} module files in {:?}",
            walk_entries,
            files.len(),
            module_files.len(),
            walk_start.elapsed()
        );
    }

    let total_files = files.len();
    if progress {
        eprintln!("Found {} files to parse...", total_files);
    }

    let mut total_count = 0;
    let parsed_global = Arc::new(AtomicUsize::new(0));

    let pool = build_thread_pool()?;
    if verbose {
        eprintln!("[verbose] thread pool built for parsing");
    }

    let root_buf = root.to_path_buf();
    let total_chunks = files.len().div_ceil(PARSE_CHUNK_SIZE);
    for (chunk_idx, chunk) in files.chunks(PARSE_CHUNK_SIZE).enumerate() {
        let root_clone = root_buf.clone();
        let counter = parsed_global.clone();
        let total = total_files;

        if verbose {
            eprintln!(
                "[verbose] chunk {}/{}: parsing {} files...",
                chunk_idx + 1,
                total_chunks,
                chunk.len()
            );
        }
        let chunk_start = Instant::now();

        // Parse chunk in parallel — at most PARSE_CHUNK_SIZE ParsedFiles in memory
        let parsed_files: Vec<ParsedFile> = pool.install(|| {
            chunk
                .par_iter()
                .filter_map(|path| {
                    let result = parse_file(&root_clone, path).ok();
                    let c = counter.fetch_add(1, Ordering::Relaxed) + 1;
                    if progress && c.is_multiple_of(PARSE_PROGRESS_INTERVAL) {
                        eprintln!("Parsed {} / {} files...", c, total);
                    }
                    result
                })
                .collect()
        });

        if verbose {
            eprintln!(
                "[verbose] chunk {}/{}: parsed in {:?}, writing {} to DB...",
                chunk_idx + 1,
                total_chunks,
                chunk_start.elapsed(),
                parsed_files.len()
            );
        }
        let write_start = Instant::now();

        // Write to DB and free parsed_files
        write_batch_to_db(conn, parsed_files, &mut total_count)?;

        if verbose {
            eprintln!(
                "[verbose] chunk {}/{}: written in {:?}",
                chunk_idx + 1,
                total_chunks,
                write_start.elapsed()
            );
        }

        if progress {
            eprintln!("Written {} / {} files to DB...", total_count, total_files);
        }
    }

    if progress {
        eprintln!("Written {} / {} files to DB", total_count, total_files);
    }

    Ok(WalkResult {
        file_count: total_count,
        module_files,
        storyboard_files,
        xcassets_dirs,
        xml_layout_files,
        res_files,
    })
}

pub(crate) fn write_batch_to_db(
    conn: &mut Connection,
    batch: Vec<ParsedFile>,
    total_count: &mut usize,
) -> Result<()> {
    let tx = conn.transaction()?;

    {
        let mut file_stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO files (path, mtime, size) VALUES (?1, ?2, ?3)",
        )?;
        let mut del_sym_stmt = tx.prepare_cached("DELETE FROM symbols WHERE file_id = ?1")?;
        let mut del_ref_stmt = tx.prepare_cached("DELETE FROM refs WHERE file_id = ?1")?;
        let mut sym_stmt = tx.prepare_cached(
            "INSERT INTO symbols (file_id, name, kind, line, signature) VALUES (?1, ?2, ?3, ?4, ?5)"
        )?;
        let mut inh_stmt = tx.prepare_cached(
            "INSERT INTO inheritance (child_id, parent_name, kind) VALUES (?1, ?2, ?3)",
        )?;
        let mut ref_stmt = tx.prepare_cached(
            "INSERT INTO refs (file_id, name, line, context) VALUES (?1, ?2, ?3, ?4)",
        )?;

        for pf in batch {
            file_stmt.execute(rusqlite::params![pf.rel_path, pf.mtime, pf.size])?;
            let file_id = tx.last_insert_rowid();

            del_sym_stmt.execute(rusqlite::params![file_id])?;
            del_ref_stmt.execute(rusqlite::params![file_id])?;

            for sym in pf.symbols {
                sym_stmt.execute(rusqlite::params![
                    file_id,
                    sym.name,
                    sym.kind.as_str(),
                    sym.line as i64,
                    sym.signature
                ])?;
                let symbol_id = tx.last_insert_rowid();

                for (parent_name, inherit_kind) in sym.parents {
                    inh_stmt.execute(rusqlite::params![symbol_id, parent_name, inherit_kind])?;
                }
            }

            for r in pf.refs {
                ref_stmt.execute(rusqlite::params![file_id, r.name, r.line as i64, r.context])?;
            }

            *total_count += 1;
        }
    }

    tx.commit()?;
    Ok(())
}

pub fn update_directory_incremental(
    conn: &mut Connection,
    root: &Path,
    progress: bool,
) -> Result<(usize, usize, usize)> {
    use ignore::WalkBuilder;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // 1. Load existing files from DB with their mtime
    let mut existing_files: HashMap<String, (i64, i64)> = HashMap::new(); // path -> (file_id, mtime)
    {
        let mut stmt = conn.prepare("SELECT id, path, mtime FROM files")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        for row in rows {
            let (id, path, mtime) = row?;
            existing_files.insert(path, (id, mtime));
        }
    }

    if progress {
        eprintln!("Loaded {} files from index", existing_files.len());
    }

    // 2. Walk filesystem and collect files to update
    let is_git = has_git_repo(root);
    let arc_root = find_arc_root(root);
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(true)
        .git_ignore(is_git)
        .filter_entry(|entry| !is_excluded_dir(entry));
    configure_walk_ignores(&mut builder, arc_root.as_deref());
    let walker = builder.build();

    let current_files: Vec<PathBuf> = walker
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(parsers::is_supported_extension)
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    // 3. Categorize files: new, changed, unchanged
    let mut files_to_parse: Vec<PathBuf> = Vec::new();
    let mut current_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    for file_path in current_files {
        let rel_path = file_path
            .strip_prefix(root)
            .unwrap_or(&file_path)
            .to_string_lossy()
            .to_string();

        let file_mtime = fs::metadata(&file_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let need_parse = if let Some((_, db_mtime)) = existing_files.get(&rel_path) {
            file_mtime > *db_mtime
        } else {
            true
        };

        if need_parse {
            files_to_parse.push(file_path);
        }
        current_paths.insert(rel_path);
    }

    // 4. Find deleted files
    let deleted_paths: Vec<String> = existing_files
        .keys()
        .filter(|p| !current_paths.contains(*p))
        .cloned()
        .collect();

    if progress {
        eprintln!(
            "Found {} new/changed files, {} deleted files",
            files_to_parse.len(),
            deleted_paths.len()
        );
    }

    // 5. Delete removed files from DB
    if !deleted_paths.is_empty() {
        let tx = conn.transaction()?;
        {
            let mut del_file_stmt = tx.prepare_cached("DELETE FROM files WHERE path = ?1")?;
            for path in &deleted_paths {
                del_file_stmt.execute(rusqlite::params![path])?;
            }
        }
        tx.commit()?;
    }

    // 6. Parse and update changed/new files
    let updated_count = if !files_to_parse.is_empty() {
        let total_files = files_to_parse.len();
        let parsed_count = Arc::new(AtomicUsize::new(0));
        let root_clone = root.to_path_buf();
        let parsed_count_clone = parsed_count.clone();

        let parsed_files: Vec<ParsedFile> = files_to_parse
            .par_iter()
            .filter_map(|path| {
                let result = parse_file(&root_clone, path).ok();
                let c = parsed_count_clone.fetch_add(1, Ordering::Relaxed) + 1;
                if progress && c.is_multiple_of(INCREMENTAL_PROGRESS_INTERVAL) {
                    eprintln!("Parsed {} / {} changed files...", c, total_files);
                }
                result
            })
            .collect();

        let count = parsed_files.len();
        let mut dummy_total = 0;
        write_batch_to_db(conn, parsed_files, &mut dummy_total)?;
        count
    } else {
        0
    };

    Ok((updated_count, files_to_parse.len(), deleted_paths.len()))
}
