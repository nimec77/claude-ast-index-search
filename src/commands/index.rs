//! Index-based search commands
//!
//! Commands for searching through the code index:
//! - search: Full-text search across files and symbols
//! - symbol: Find symbol by name
//! - class: Find class by name
//! - implementations: Find implementations of interface/class
//! - hierarchy: Show class hierarchy
//! - usages: Find symbol usages (indexed or grep-based)

use std::collections::HashSet;
use std::path::Path;

use anyhow::Result;
use colored::Colorize;

use super::common::CommandTimer;
use regex::Regex;
use rusqlite::{Connection, params};

use super::{relative_path, search_files};
use crate::db::{self, SearchScope};
use crate::open_db_or_return;

/// Full-text search across files, symbols, and file contents
pub fn cmd_search(
    root: &Path,
    query: &str,
    limit: usize,
    format: &str,
    scope: &SearchScope,
    fuzzy: bool,
) -> Result<()> {
    let _timer = CommandTimer::new();

    let conn = open_db_or_return!(root);

    // Support comma-separated OR queries
    let terms: Vec<&str> = query
        .split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();

    let mut files: Vec<String> = Vec::new();
    let mut symbols: Vec<db::SearchResult> = Vec::new();
    let mut ref_matches: Vec<(String, i64)> = Vec::new();
    let mut content_matches: Vec<(String, usize, String)> = Vec::new();

    // Dedup sets
    let mut seen_files: HashSet<String> = HashSet::new();
    let mut seen_symbols: HashSet<(String, i64, String)> = HashSet::new(); // (name, line, path)
    let mut seen_refs: HashSet<String> = HashSet::new();
    let mut seen_content: HashSet<(String, usize)> = HashSet::new(); // (path, line)

    // 1. Search in file paths (index)
    for term in &terms {
        for f in db::find_files(&conn, term, limit)? {
            if let Some(prefix) = scope.dir_prefix
                && !f.starts_with(prefix)
            {
                continue;
            }
            if seen_files.insert(f.clone()) {
                files.push(f);
            }
        }
    }

    // 2. Search in symbols using FTS or fuzzy (index)
    for term in &terms {
        let batch = if fuzzy {
            db::search_symbols_fuzzy(&conn, term, limit)?
        } else {
            let fts_query = format!("{}*", term);
            db::search_symbols_scoped(&conn, &fts_query, limit, scope)?
        };
        for s in batch {
            if seen_symbols.insert((s.name.clone(), s.line, s.path.clone())) {
                symbols.push(s);
            }
        }
    }

    // 3. Search in references (imports and usages from index)
    for term in &terms {
        for r in db::search_refs(&conn, term, limit)? {
            if seen_refs.insert(r.0.clone()) {
                ref_matches.push(r);
            }
        }
    }

    // 4. Search in file contents (grep)
    let pattern = terms
        .iter()
        .map(|t| regex::escape(t))
        .collect::<Vec<_>>()
        .join("|");

    super::search_files_limited(
        root,
        &pattern,
        super::grep::ALL_SOURCE_EXTENSIONS,
        limit,
        |path, line_num, line| {
            let rel_path = super::relative_path(root, path);
            // Apply scope filter for grep results
            if let Some(prefix) = scope.dir_prefix
                && !rel_path.starts_with(prefix)
            {
                return;
            }
            if let Some(in_file) = scope.in_file
                && !rel_path.contains(in_file)
            {
                return;
            }
            if let Some(module) = scope.module
                && !rel_path.starts_with(module)
            {
                return;
            }
            if seen_content.insert((rel_path.clone(), line_num)) {
                let content: String = line.trim().chars().take(100).collect();
                content_matches.push((rel_path, line_num, content));
            }
        },
    )?;

    if format == "json" {
        let result = serde_json::json!({
            "files": files,
            "symbols": symbols,
            "references": ref_matches.iter().map(|(name, count)| {
                serde_json::json!({"name": name, "usage_count": count})
            }).collect::<Vec<_>>(),
            "content_matches": content_matches.iter().map(|(p, l, c)| {
                serde_json::json!({"path": p, "line": l, "content": c})
            }).collect::<Vec<_>>()
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    // Output results
    println!("{}", format!("Search results for '{}':", query).bold());

    if !files.is_empty() {
        println!("\n{}", "Files (by path):".cyan());
        for path in files.iter().take(limit) {
            println!("  {}", path);
        }
        if files.len() > limit {
            println!("  ... and {} more", files.len() - limit);
        }
    }

    if !symbols.is_empty() {
        println!("\n{}", "Symbols (definitions):".cyan());
        for s in symbols.iter().take(limit) {
            println!("  {} [{}]: {}:{}", s.name.cyan(), s.kind, s.path, s.line);
        }
    }

    if !ref_matches.is_empty() {
        println!("\n{}", "References (imports & usages):".cyan());
        for (name, count) in ref_matches.iter().take(limit) {
            println!("  {} — used in {} places", name.cyan(), count);
        }
    }

    if !content_matches.is_empty() {
        println!("\n{}", "Content matches:".cyan());
        for (path, line_num, content) in content_matches.iter().take(limit) {
            println!("  {}:{}", path.cyan(), line_num);
            println!("    {}", content.dimmed());
        }
        if content_matches.len() > limit {
            println!("  ... and {} more", content_matches.len() - limit);
        }
    }

    if files.is_empty()
        && symbols.is_empty()
        && ref_matches.is_empty()
        && content_matches.is_empty()
    {
        println!("  No results found.");
    }

    Ok(())
}

/// Find symbol by name or glob pattern
#[allow(clippy::too_many_arguments)]
pub fn cmd_symbol(
    root: &Path,
    name: Option<&str>,
    pattern: Option<&str>,
    kind: Option<&str>,
    limit: usize,
    format: &str,
    scope: &SearchScope,
    fuzzy: bool,
) -> Result<()> {
    let _timer = CommandTimer::new();

    let conn = open_db_or_return!(root);

    let (symbols, display_name) = if let Some(pat) = pattern {
        (
            db::find_symbols_by_pattern(&conn, pat, kind, limit)?,
            pat.to_string(),
        )
    } else {
        let name = name.unwrap_or("");
        let results = if fuzzy && kind.is_none() {
            db::search_symbols_fuzzy(&conn, name, limit)?
        } else {
            db::find_symbols_by_name_scoped(&conn, name, kind, limit, scope)?
        };
        (results, name.to_string())
    };

    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&symbols)?);
        return Ok(());
    }

    let kind_str = kind.map(|k| format!(" ({})", k)).unwrap_or_default();
    println!(
        "{}",
        format!("Symbols matching '{}'{}:", display_name, kind_str).bold()
    );

    for s in &symbols {
        println!("  {} [{}]: {}:{}", s.name.cyan(), s.kind, s.path, s.line);
        if let Some(sig) = &s.signature {
            let truncated: String = sig.chars().take(70).collect();
            println!("    {}", truncated.dimmed());
        }
    }

    if symbols.is_empty() {
        println!("  No symbols found.");
    }

    Ok(())
}

/// Find class by name or glob pattern (classes, interfaces, objects, enums)
pub fn cmd_class(
    root: &Path,
    name: Option<&str>,
    pattern: Option<&str>,
    limit: usize,
    format: &str,
    scope: &SearchScope,
    fuzzy: bool,
) -> Result<()> {
    let _timer = CommandTimer::new();

    let conn = open_db_or_return!(root);

    let (results, display_name) = if let Some(pat) = pattern {
        (
            db::find_class_like_pattern(&conn, pat, limit)?,
            pat.to_string(),
        )
    } else {
        let name = name.unwrap_or("");
        let r = if fuzzy {
            let all = db::search_symbols_fuzzy(&conn, name, limit * 5)?;
            all.into_iter()
                .filter(|s| {
                    matches!(
                        s.kind.as_str(),
                        "class"
                            | "interface"
                            | "object"
                            | "enum"
                            | "protocol"
                            | "struct"
                            | "actor"
                            | "package"
                    )
                })
                .take(limit)
                .collect()
        } else {
            db::find_class_like_scoped(&conn, name, limit, scope)?
        };
        (r, name.to_string())
    };

    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    println!("{}", format!("Classes matching '{}':", display_name).bold());

    for s in &results {
        println!("  {} [{}]: {}:{}", s.name.cyan(), s.kind, s.path, s.line);
    }

    if results.is_empty() {
        println!("  No classes found.");
    }

    Ok(())
}

/// Find implementations of interface/class
pub fn cmd_implementations(
    root: &Path,
    parent: &str,
    limit: usize,
    format: &str,
    scope: &SearchScope,
) -> Result<()> {
    let _timer = CommandTimer::new();

    let conn = open_db_or_return!(root);
    let impls = if scope.is_empty() {
        db::find_implementations(&conn, parent, limit)?
    } else {
        // For scoped implementations, filter results post-query
        let all = db::find_implementations(&conn, parent, limit * 5)?;
        all.into_iter()
            .filter(|s| {
                if let Some(in_file) = scope.in_file
                    && !s.path.contains(in_file)
                {
                    return false;
                }
                if let Some(module) = scope.module
                    && !s.path.starts_with(module)
                {
                    return false;
                }
                true
            })
            .take(limit)
            .collect()
    };

    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&impls)?);
        return Ok(());
    }

    println!("{}", format!("Implementations of '{}':", parent).bold());

    for s in &impls {
        println!("  {} [{}]: {}:{}", s.name.cyan(), s.kind, s.path, s.line);
    }

    if impls.is_empty() {
        println!("  No implementations found.");
    }

    Ok(())
}

/// Show cross-references: definitions, imports, usages
pub fn cmd_refs(root: &Path, symbol: &str, limit: usize, format: &str) -> Result<()> {
    let _timer = CommandTimer::new();

    let conn = open_db_or_return!(root);
    let (definitions, imports, usages) = db::find_cross_references(&conn, symbol, limit)?;

    if format == "json" {
        let result = serde_json::json!({
            "definitions": definitions,
            "imports": imports,
            "usages": usages,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!("{}", format!("Cross-references for '{}':", symbol).bold());

    if !definitions.is_empty() {
        println!("\n  {}", "Definitions:".cyan());
        for s in &definitions {
            println!("    {} [{}]: {}:{}", s.name.cyan(), s.kind, s.path, s.line);
        }
    }

    if !imports.is_empty() {
        println!("\n  {}", "Imports:".cyan());
        for s in &imports {
            println!("    {}:{}", s.path.cyan(), s.line);
            if let Some(sig) = &s.signature {
                println!("      {}", sig.dimmed());
            }
        }
    }

    if !usages.is_empty() {
        println!("\n  {}", "Usages:".cyan());
        for r in &usages {
            println!("    {}:{}", r.path.cyan(), r.line);
            if let Some(ctx) = &r.context {
                let truncated: String = ctx.chars().take(80).collect();
                println!("      {}", truncated.dimmed());
            }
        }
    }

    if definitions.is_empty() && imports.is_empty() && usages.is_empty() {
        println!("  No references found.");
    }

    Ok(())
}

/// Show class hierarchy (parents and children)
pub fn cmd_hierarchy(root: &Path, name: &str) -> Result<()> {
    let _timer = CommandTimer::new();

    let conn = open_db_or_return!(root);

    // Find the class/interface/package
    let classes = db::find_symbols_by_name(&conn, name, Some("class"), 1)?;
    let interfaces = db::find_symbols_by_name(&conn, name, Some("interface"), 1)?;
    let packages = db::find_symbols_by_name(&conn, name, Some("package"), 1)?;
    let protocols = db::find_symbols_by_name(&conn, name, Some("protocol"), 1)?;

    let target = classes
        .first()
        .or(interfaces.first())
        .or(packages.first())
        .or(protocols.first());

    if target.is_none() {
        println!("{}", format!("Class '{}' not found.", name).red());
        return Ok(());
    }

    println!("{}", format!("Hierarchy for '{}':", name).bold());

    // Find parents
    let mut stmt = conn.prepare(
        "SELECT i.parent_name, i.kind FROM inheritance i JOIN symbols s ON i.child_id = s.id WHERE s.name = ?1",
    )?;
    let parents: Vec<(String, String)> = stmt
        .query_map([name], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;

    if !parents.is_empty() {
        println!("\n  {}", "Parents:".cyan());
        for (parent, kind) in &parents {
            println!("    {} ({})", parent, kind);
        }
    }

    // Find children
    let children = db::find_implementations(&conn, name, 20)?;
    if !children.is_empty() {
        println!("\n  {}", "Children:".cyan());
        for c in &children {
            println!("    {} [{}]", c.name, c.kind);
        }
    }

    Ok(())
}

/// Find symbol usages (indexed or grep-based)
pub fn cmd_usages(
    root: &Path,
    symbol: &str,
    limit: usize,
    format: &str,
    scope: &SearchScope,
) -> Result<()> {
    let _timer = CommandTimer::new();

    // Try to use index first
    let db_path = db::get_db_path(root)?;
    if db_path.exists() {
        let conn = Connection::open(&db_path)?;

        // Check if refs table has data
        let refs_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM refs WHERE name = ?1 LIMIT 1",
                params![symbol],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if refs_count > 0 {
            // Use indexed references with scope filtering
            let refs = db::find_references_scoped(&conn, symbol, limit, scope)?;

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&refs)?);
                return Ok(());
            }

            println!(
                "{}",
                format!("Usages of '{}' ({}):", symbol, refs.len()).bold()
            );

            for r in &refs {
                println!("  {}:{}", r.path.cyan(), r.line);
                if let Some(ctx) = &r.context {
                    let truncated: String = ctx.chars().take(80).collect();
                    println!("    {}", truncated);
                }
            }

            if refs.is_empty() {
                println!("  No usages found in index.");
            }

            return Ok(());
        }
    }

    // Fallback to grep-based search
    let pattern = format!(r"\b{}\b", regex::escape(symbol));
    let def_pattern = Regex::new(&format!(
        r"(class|interface|object|fun|val|var|typealias)\s+{}\b",
        regex::escape(symbol)
    ))?;

    let mut usages: Vec<(String, usize, String)> = vec![];

    search_files(root, &pattern, &["kt", "java"], |path, line_num, line| {
        if usages.len() >= limit {
            return;
        }

        // Skip definitions
        if def_pattern.is_match(line) {
            return;
        }

        let rel_path = relative_path(root, path);
        // Apply scope filter for grep results
        if let Some(in_file) = scope.in_file
            && !rel_path.contains(in_file)
        {
            return;
        }
        if let Some(module) = scope.module
            && !rel_path.starts_with(module)
        {
            return;
        }
        let content: String = line.trim().chars().take(80).collect();
        usages.push((rel_path, line_num, content));
    })?;

    if format == "json" {
        let result: Vec<_> = usages
            .iter()
            .map(|(p, l, c)| serde_json::json!({"path": p, "line": l, "content": c}))
            .collect();
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!(
        "{}",
        format!("Usages of '{}' ({}):", symbol, usages.len()).bold()
    );

    for (path, line_num, content) in &usages {
        println!("  {}:{}", path.cyan(), line_num);
        println!("    {}", content);
    }

    if usages.is_empty() {
        println!("  No usages found.");
    }

    Ok(())
}
