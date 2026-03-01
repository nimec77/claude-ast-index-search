//! Perl-specific commands
//!
//! Commands for working with Perl codebases:
//! - perl_exports: Find @EXPORT/@EXPORT_OK definitions
//! - perl_subs: Find subroutine definitions
//! - perl_pod: Find POD documentation
//! - perl_tests: Find test assertions
//! - perl_imports: Find use/require statements

use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use colored::Colorize;

use super::{relative_path, search_files_limited};

/// Search files for a pattern and print results in the standard two-line format.
///
/// For each match the query filter (if any) is applied, then the optional
/// `extra_filter` predicate. Results are printed as:
///   label (N):
///     path:line_num
///       content (truncated to truncate_len)
// Extra_filter is the 8th argument; all are required to avoid a struct wrapper for this helper.
#[allow(clippy::too_many_arguments)]
fn grep_and_print(
    root: &Path,
    pattern: &str,
    extensions: &[&str],
    query: Option<&str>,
    limit: usize,
    label: &str,
    truncate_len: usize,
    extra_filter: impl Fn(&str) -> bool,
) -> Result<()> {
    let start = Instant::now();
    let mut results: Vec<(String, usize, String)> = vec![];

    search_files_limited(root, pattern, extensions, limit, |path, line_num, line| {
        if !extra_filter(line) {
            return;
        }
        if let Some(q) = query
            && !line.to_lowercase().contains(&q.to_lowercase())
        {
            return;
        }
        let rel_path = relative_path(root, path);
        let content: String = line.trim().chars().take(truncate_len).collect();
        results.push((rel_path, line_num, content));
    })?;

    println!("{}", format!("{} ({}):", label, results.len()).bold());
    for (path, line_num, content) in &results {
        println!("  {}:{}", path.cyan(), line_num);
        println!("    {}", content);
    }
    eprintln!("\n{}", format!("Time: {:?}", start.elapsed()).dimmed());
    Ok(())
}

/// Find Perl @EXPORT and @EXPORT_OK definitions
pub fn cmd_perl_exports(root: &Path, query: Option<&str>, limit: usize) -> Result<()> {
    grep_and_print(
        root,
        r"our\s+@EXPORT|our\s+@EXPORT_OK|@EXPORT\s*=|@EXPORT_OK\s*=",
        &["pm"],
        query,
        limit,
        "Perl exports",
        100,
        |_| true,
    )
}

/// Find Perl subroutine definitions
pub fn cmd_perl_subs(root: &Path, query: Option<&str>, limit: usize) -> Result<()> {
    grep_and_print(
        root,
        r"^\s*sub\s+\w+",
        &["pm", "pl", "t"],
        query,
        limit,
        "Perl subroutines",
        80,
        |_| true,
    )
}

/// Find POD documentation sections
pub fn cmd_perl_pod(root: &Path, query: Option<&str>, limit: usize) -> Result<()> {
    grep_and_print(
        root,
        r"^=(head[1-4]|item|over|back|pod|cut|begin|end|for)\b",
        &["pm", "pl", "pod"],
        query,
        limit,
        "POD documentation",
        100,
        |_| true,
    )
}

/// Find Perl test assertions (Test::More, Test::Simple)
pub fn cmd_perl_tests(root: &Path, query: Option<&str>, limit: usize) -> Result<()> {
    grep_and_print(
        root,
        r"\b(ok|is|isnt|like|unlike|cmp_ok|is_deeply|diag|pass|fail|subtest|plan|done_testing|SKIP|TODO)\s*[\(\{]",
        &["t", "pm", "pl"],
        query,
        limit,
        "Perl tests",
        100,
        |_| true,
    )
}

/// Find Perl use/require statements
pub fn cmd_perl_imports(root: &Path, query: Option<&str>, limit: usize) -> Result<()> {
    grep_and_print(
        root,
        r"^\s*(use|require)\s+[A-Za-z]",
        &["pm", "pl", "t"],
        query,
        limit,
        "Perl imports",
        100,
        |line| {
            // Skip 'use strict', 'use warnings', 'use constant', 'use base', 'use parent'
            let trimmed = line.trim();
            !trimmed.starts_with("use strict")
                && !trimmed.starts_with("use warnings")
                && !trimmed.starts_with("use constant")
                && !trimmed.starts_with("use base")
                && !trimmed.starts_with("use parent")
                && !trimmed.starts_with("use utf8")
                && !trimmed.starts_with("use v5")
                && !trimmed.starts_with("use 5.")
        },
    )
}
