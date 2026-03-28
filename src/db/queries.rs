//! Query types and search functions for the symbol index.

use anyhow::Result;
use rusqlite::{Connection, params};
use serde::Serialize;

/// Symbol kinds that represent class-like declarations.
const CLASS_LIKE_KINDS: &[&str] = &[
    "class",
    "interface",
    "object",
    "enum",
    "protocol",
    "struct",
    "actor",
    "package",
];

/// Build a SQL `IN (...)` clause for class-like kinds.
fn class_like_in_clause() -> String {
    let items: Vec<String> = CLASS_LIKE_KINDS
        .iter()
        .map(|k| format!("'{}'", k))
        .collect();
    format!("({})", items.join(", "))
}

/// Collect boxed query params from: initial string values, scope params, and a trailing limit.
fn collect_query_params(
    initial: &[&str],
    scope_params: &[String],
    limit: usize,
) -> Vec<Box<dyn rusqlite::types::ToSql>> {
    let mut all: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    for p in initial {
        all.push(Box::new(p.to_string()));
    }
    for p in scope_params {
        all.push(Box::new(p.clone()));
    }
    all.push(Box::new(limit as i64));
    all
}

/// Convert `Vec<Box<dyn ToSql>>` into reference slice for rusqlite.
fn params_as_refs(params: &[Box<dyn rusqlite::types::ToSql>]) -> Vec<&dyn rusqlite::types::ToSql> {
    params.iter().map(|p| p.as_ref()).collect()
}

/// Escape FTS5 special characters
#[cfg_attr(test, allow(dead_code))]
pub(super) fn escape_fts5_query(query: &str) -> String {
    // Handle empty query
    if query.trim().is_empty() {
        return String::new();
    }
    // Check for prefix operator: * must stay OUTSIDE quotes for FTS5
    let (term, suffix) = if let Some(stripped) = query.strip_suffix('*') {
        (stripped, "*")
    } else {
        (query, "")
    };
    // Wrap in double quotes to treat as literal phrase
    // Escape any existing double quotes
    let escaped = term.replace('"', "\"\"");
    format!("\"{}\"{}", escaped, suffix)
}

/// Search symbols by name (FTS5). Thin wrapper around `search_symbols_scoped`.
pub fn search_symbols(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    search_symbols_scoped(conn, query, limit, &SearchScope::empty())
}

/// Search result
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub name: String,
    pub kind: String,
    pub line: i64,
    pub signature: Option<String>,
    pub path: String,
}

impl SearchResult {
    /// Map a database row to a `SearchResult`.
    /// Expects columns in order: name, kind, line, signature, path.
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(SearchResult {
            name: row.get(0)?,
            kind: row.get(1)?,
            line: row.get(2)?,
            signature: row.get(3)?,
            path: row.get(4)?,
        })
    }
}

/// Find files by name pattern
pub fn find_files(conn: &Connection, pattern: &str, limit: usize) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT path FROM files WHERE path LIKE ?1 LIMIT ?2")?;

    let pattern = format!("%{}%", pattern);
    let results = stmt
        .query_map(params![pattern, limit as i64], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}

/// Find symbols by name (exact match first, then prefix/contains if no results).
/// Thin wrapper around `find_symbols_by_name_scoped` with an empty scope.
pub fn find_symbols_by_name(
    conn: &Connection,
    name: &str,
    kind: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    find_symbols_by_name_scoped(conn, name, kind, limit, &SearchScope::empty())
}

/// Find class-like symbols (class, interface, object, enum) by name - single query.
/// Thin wrapper around `find_class_like_scoped` with an empty scope.
pub fn find_class_like(conn: &Connection, name: &str, limit: usize) -> Result<Vec<SearchResult>> {
    find_class_like_scoped(conn, name, limit, &SearchScope::empty())
}

/// Find implementations (subclasses/implementors)
pub fn find_implementations(
    conn: &Connection,
    parent_name: &str,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    // Match exact name or qualified suffix (%.Name) only.
    // Avoids broad contains match (%Name%) which causes false positives
    // (e.g., searching "Map" would match "HashMap", "TreeMap").
    let suffix_pattern = format!("%.{}", parent_name);
    let mut stmt = conn.prepare(
        r#"
        SELECT s.name, s.kind, s.line, s.signature, f.path
        FROM inheritance i
        JOIN symbols s ON i.child_id = s.id
        JOIN files f ON s.file_id = f.id
        WHERE i.parent_name = ?1 OR i.parent_name LIKE ?2
        ORDER BY
            CASE
                WHEN i.parent_name = ?1 THEN 0
                ELSE 1
            END, s.name
        LIMIT ?3
        "#,
    )?;

    let results = stmt
        .query_map(
            params![parent_name, suffix_pattern, limit as i64],
            SearchResult::from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}

/// Get database statistics
pub fn get_stats(conn: &Connection) -> Result<DbStats> {
    conn.query_row(
        r#"
        SELECT
            (SELECT COUNT(*) FROM files),
            (SELECT COUNT(*) FROM symbols),
            (SELECT COUNT(*) FROM modules),
            COALESCE((SELECT COUNT(*) FROM refs), 0),
            COALESCE((SELECT COUNT(*) FROM xml_usages), 0),
            COALESCE((SELECT COUNT(*) FROM resources), 0),
            COALESCE((SELECT COUNT(*) FROM storyboard_usages), 0),
            COALESCE((SELECT COUNT(*) FROM ios_assets), 0)
        "#,
        [],
        |row| {
            Ok(DbStats {
                file_count: row.get(0)?,
                symbol_count: row.get(1)?,
                module_count: row.get(2)?,
                refs_count: row.get(3)?,
                xml_usages_count: row.get(4)?,
                resources_count: row.get(5)?,
                storyboard_usages_count: row.get(6)?,
                ios_assets_count: row.get(7)?,
            })
        },
    )
    .map_err(Into::into)
}

#[derive(Debug, Serialize)]
pub struct DbStats {
    pub file_count: i64,
    pub symbol_count: i64,
    pub module_count: i64,
    pub refs_count: i64,
    pub xml_usages_count: i64,
    pub resources_count: i64,
    pub storyboard_usages_count: i64,
    pub ios_assets_count: i64,
}

/// Reference result
#[derive(Debug, Serialize)]
pub struct RefResult {
    pub name: String,
    pub line: i64,
    pub context: Option<String>,
    pub path: String,
}

impl RefResult {
    /// Map a database row to a `RefResult`.
    /// Expects columns in order: name, line, context, path.
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(RefResult {
            name: row.get(0)?,
            line: row.get(1)?,
            context: row.get(2)?,
            path: row.get(3)?,
        })
    }
}

/// Find references (usages) of a symbol.
/// Thin wrapper around `find_references_scoped` with an empty scope.
pub fn find_references(conn: &Connection, name: &str, limit: usize) -> Result<Vec<RefResult>> {
    find_references_scoped(conn, name, limit, &SearchScope::empty())
}

/// Search references by name (prefix match, grouped by unique name)
pub fn search_refs(conn: &Connection, query: &str, limit: usize) -> Result<Vec<(String, i64)>> {
    let pattern = format!("{}%", query);
    let mut stmt = conn.prepare(
        r#"
        SELECT r.name, COUNT(*) as usage_count
        FROM refs r
        WHERE r.name LIKE ?1
        GROUP BY r.name
        ORDER BY
            CASE WHEN r.name = ?2 THEN 0
                 WHEN r.name LIKE ?1 THEN 1
                 ELSE 2
            END,
            usage_count DESC
        LIMIT ?3
        "#,
    )?;
    let results = stmt
        .query_map(params![pattern, query, limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(results)
}

/// Count references in the database
pub fn count_refs(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM refs", [], |row| row.get(0))?)
}

/// Find import statements for a symbol name
pub fn find_imports(conn: &Connection, name: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT s.name, s.kind, s.line, s.signature, f.path
        FROM symbols s
        JOIN files f ON s.file_id = f.id
        WHERE s.kind = 'import' AND s.name = ?1
        LIMIT ?2
        "#,
    )?;

    let results = stmt
        .query_map(params![name, limit as i64], SearchResult::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}

/// Find all cross-references for a symbol: definitions, imports, and usages
pub fn find_cross_references(
    conn: &Connection,
    name: &str,
    limit: usize,
) -> Result<(Vec<SearchResult>, Vec<SearchResult>, Vec<RefResult>)> {
    // 1. Definitions (non-import symbols)
    let definitions = find_symbols_by_name(conn, name, None, limit)?
        .into_iter()
        .filter(|s| s.kind != "import")
        .collect();

    // 2. Imports
    let imports = find_imports(conn, name, limit)?;

    // 3. Usages (refs table)
    let usages = find_references(conn, name, limit)?;

    Ok((definitions, imports, usages))
}

/// Fuzzy search for symbols: exact → prefix → contains cascade
pub fn search_symbols_fuzzy(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    // Single query: contains match with ranking by relevance
    // exact match (name = query) first, then prefix, then contains — sorted by length
    let contains_pattern = format!("%{}%", query);
    let mut stmt = conn.prepare(
        r#"
        SELECT s.name, s.kind, s.line, s.signature, f.path
        FROM symbols s
        JOIN files f ON s.file_id = f.id
        WHERE s.name LIKE ?1
        ORDER BY
            CASE WHEN s.name = ?2 THEN 0
                 WHEN s.name LIKE ?3 THEN 1
                 ELSE 2 END,
            length(s.name)
        LIMIT ?4
        "#,
    )?;
    let prefix_pattern = format!("{}%", query);
    let results: Vec<SearchResult> = stmt
        .query_map(
            params![contains_pattern, query, prefix_pattern, limit as i64],
            SearchResult::from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}

/// Scope filter for narrowing search results by file path or module
pub struct SearchScope<'a> {
    pub in_file: Option<&'a str>,
    pub module: Option<&'a str>,
    /// Directory prefix filter: only return results under this path (relative to project root)
    pub dir_prefix: Option<&'a str>,
}

impl SearchScope<'_> {
    pub fn empty() -> Self {
        SearchScope {
            in_file: None,
            module: None,
            dir_prefix: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.in_file.is_none() && self.module.is_none() && self.dir_prefix.is_none()
    }

    /// Build WHERE clause fragment and collect params
    fn path_condition(&self) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut params = Vec::new();
        if let Some(prefix) = self.dir_prefix {
            conditions.push("f.path LIKE ?".to_string());
            params.push(format!("{}%", prefix));
        }
        if let Some(file) = self.in_file {
            conditions.push("f.path LIKE ?".to_string());
            params.push(format!("%{}%", file));
        }
        if let Some(module) = self.module {
            conditions.push("f.path LIKE ?".to_string());
            params.push(format!("{}%", module));
        }
        if conditions.is_empty() {
            (String::new(), params)
        } else {
            (format!(" AND {}", conditions.join(" AND ")), params)
        }
    }
}

/// Search symbols with scope filtering (file/module)
pub fn search_symbols_scoped(
    conn: &Connection,
    query: &str,
    limit: usize,
    scope: &SearchScope,
) -> Result<Vec<SearchResult>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let escaped_query = escape_fts5_query(query);
    let (scope_clause, scope_params) = scope.path_condition();

    let sql = format!(
        r#"
        SELECT s.name, s.kind, s.line, s.signature, f.path
        FROM symbols_fts fts
        JOIN symbols s ON fts.rowid = s.id
        JOIN files f ON s.file_id = f.id
        WHERE symbols_fts MATCH ?1{}
        LIMIT ?{}
        "#,
        scope_clause,
        2 + scope_params.len()
    );

    let mut stmt = conn.prepare(&sql)?;
    let all_params = collect_query_params(&[&escaped_query], &scope_params, limit);
    let param_refs = params_as_refs(&all_params);
    let results = stmt
        .query_map(param_refs.as_slice(), SearchResult::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}

/// Find symbols by name with scope filtering (exact match first, then prefix/contains if no results).
/// Non-scoped callers should use `find_symbols_by_name` which delegates here with an empty scope.
pub fn find_symbols_by_name_scoped(
    conn: &Connection,
    name: &str,
    kind: Option<&str>,
    limit: usize,
    scope: &SearchScope,
) -> Result<Vec<SearchResult>> {
    let (scope_clause, scope_params) = scope.path_condition();

    // Try exact match first
    let mut exact_sql = format!(
        "SELECT s.name, s.kind, s.line, s.signature, f.path FROM symbols s JOIN files f ON s.file_id = f.id WHERE s.name = ?1{}",
        scope_clause
    );
    if kind.is_some() {
        exact_sql.push_str(&format!(" AND s.kind = ?{}", 2 + scope_params.len()));
        exact_sql.push_str(&format!(" LIMIT ?{}", 3 + scope_params.len()));
    } else {
        exact_sql.push_str(&format!(" LIMIT ?{}", 2 + scope_params.len()));
    }

    let mut all_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    all_params.push(Box::new(name.to_string()));
    for p in &scope_params {
        all_params.push(Box::new(p.clone()));
    }
    if let Some(k) = kind {
        all_params.push(Box::new(k.to_string()));
    }
    all_params.push(Box::new(limit as i64));

    let mut stmt = conn.prepare(&exact_sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        all_params.iter().map(|p| p.as_ref()).collect();
    let results = stmt
        .query_map(param_refs.as_slice(), SearchResult::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    // If no exact match, try prefix match
    if results.is_empty() {
        let pattern = format!("{}%", name);
        let mut prefix_sql = format!(
            "SELECT s.name, s.kind, s.line, s.signature, f.path FROM symbols s JOIN files f ON s.file_id = f.id WHERE s.name LIKE ?1{}",
            scope_clause
        );
        if kind.is_some() {
            prefix_sql.push_str(&format!(" AND s.kind = ?{}", 2 + scope_params.len()));
            prefix_sql.push_str(" ORDER BY length(s.name)");
            prefix_sql.push_str(&format!(" LIMIT ?{}", 3 + scope_params.len()));
        } else {
            prefix_sql.push_str(" ORDER BY length(s.name)");
            prefix_sql.push_str(&format!(" LIMIT ?{}", 2 + scope_params.len()));
        }

        let mut all_params2: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        all_params2.push(Box::new(pattern));
        for p in &scope_params {
            all_params2.push(Box::new(p.clone()));
        }
        if let Some(k) = kind {
            all_params2.push(Box::new(k.to_string()));
        }
        all_params2.push(Box::new(limit as i64));

        let mut stmt2 = conn.prepare(&prefix_sql)?;
        let param_refs2: Vec<&dyn rusqlite::types::ToSql> =
            all_params2.iter().map(|p| p.as_ref()).collect();
        let prefix_results = stmt2
            .query_map(param_refs2.as_slice(), SearchResult::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(prefix_results);
    }

    Ok(results)
}

/// Find class-like symbols with scope filtering.
/// Non-scoped callers should use `find_class_like` which delegates here with an empty scope.
pub fn find_class_like_scoped(
    conn: &Connection,
    name: &str,
    limit: usize,
    scope: &SearchScope,
) -> Result<Vec<SearchResult>> {
    let (scope_clause, scope_params) = scope.path_condition();

    let kinds = class_like_in_clause();
    let sql = format!(
        r#"
        SELECT s.name, s.kind, s.line, s.signature, f.path
        FROM symbols s
        JOIN files f ON s.file_id = f.id
        WHERE s.name = ?1 AND s.kind IN {kinds}{}
        LIMIT ?{}
        "#,
        scope_clause,
        2 + scope_params.len()
    );

    let mut stmt = conn.prepare(&sql)?;
    let all_params = collect_query_params(&[name], &scope_params, limit);
    let param_refs = params_as_refs(&all_params);
    let results = stmt
        .query_map(param_refs.as_slice(), SearchResult::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}

/// Convert a glob pattern to SQL LIKE pattern: `*` → `%`, `?` → `_`
pub fn glob_to_like(pattern: &str) -> String {
    pattern
        .replace('%', r"\%")
        .replace('_', r"\_")
        .replace('*', "%")
        .replace('?', "_")
}

/// Find symbols by glob pattern with optional kind filter
pub fn find_symbols_by_pattern(
    conn: &Connection,
    pattern: &str,
    kind: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    let like_pattern = glob_to_like(pattern);
    let mut sql = String::from(
        "SELECT s.name, s.kind, s.line, s.signature, f.path \
         FROM symbols s JOIN files f ON s.file_id = f.id \
         WHERE s.name LIKE ?1",
    );
    if kind.is_some() {
        sql.push_str(" AND s.kind = ?2 ORDER BY length(s.name) LIMIT ?3");
    } else {
        sql.push_str(" ORDER BY length(s.name) LIMIT ?2");
    }

    let mut stmt = conn.prepare(&sql)?;
    let results = if let Some(k) = kind {
        stmt.query_map(
            params![like_pattern, k, limit as i64],
            SearchResult::from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map(params![like_pattern, limit as i64], SearchResult::from_row)?
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(results)
}

/// Find class-like symbols by glob pattern
pub fn find_class_like_pattern(
    conn: &Connection,
    pattern: &str,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    let like_pattern = glob_to_like(pattern);
    let kinds = class_like_in_clause();
    let sql = format!(
        r#"
        SELECT s.name, s.kind, s.line, s.signature, f.path
        FROM symbols s
        JOIN files f ON s.file_id = f.id
        WHERE s.name LIKE ?1 AND s.kind IN {kinds}
        ORDER BY length(s.name)
        LIMIT ?2
        "#,
    );
    let mut stmt = conn.prepare(&sql)?;
    let results = stmt
        .query_map(params![like_pattern, limit as i64], SearchResult::from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(results)
}

/// Find references with scope filtering
pub fn find_references_scoped(
    conn: &Connection,
    name: &str,
    limit: usize,
    scope: &SearchScope,
) -> Result<Vec<RefResult>> {
    let (scope_clause, scope_params) = scope.path_condition();

    let sql = format!(
        r#"
        SELECT r.name, r.line, r.context, f.path
        FROM refs r
        JOIN files f ON r.file_id = f.id
        WHERE r.name = ?1{}
        ORDER BY f.path, r.line
        LIMIT ?{}
        "#,
        scope_clause,
        2 + scope_params.len()
    );

    let mut stmt = conn.prepare(&sql)?;
    let all_params = collect_query_params(&[name], &scope_params, limit);
    let param_refs = params_as_refs(&all_params);
    let results = stmt
        .query_map(param_refs.as_slice(), RefResult::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}
