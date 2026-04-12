mod queries;

pub use queries::*;

use anyhow::{Context, Result};
use colored::Colorize;
use rusqlite::{Connection, params};
use std::fs::File;
use std::path::{Path, PathBuf};

// --- Constants ---

/// Database file suffixes (main DB + WAL mode files)
const DB_FILE_SUFFIXES: &[&str] = &["index.db", "index.db-wal", "index.db-shm"];

/// Cache directory name for the index
const CACHE_DIR_NAME: &str = "ast-index";

/// Legacy cache directory name (pre-rename)
const LEGACY_CACHE_DIR_NAME: &str = "kotlin-index";

/// djb2 hash seed value
const DJB2_SEED: u64 = 5381;

/// SQLite cache size in KB (negative = KB, positive = pages). 8 MB.
const SQLITE_CACHE_SIZE: &str = "-8000";

/// SQLite busy timeout in milliseconds
const SQLITE_BUSY_TIMEOUT_MS: i64 = 5000;

/// Environment variable name for DB path override
const ENV_DB_PATH: &str = "AST_INDEX_DB_PATH";

/// Legacy environment variable name
const ENV_DB_PATH_LEGACY: &str = "KOTLIN_INDEX_DB_PATH";

/// Get the database path for the current project
pub fn get_db_path(project_root: &Path) -> Result<PathBuf> {
    // Check env: new name first, fallback to old
    if let Ok(path) = std::env::var(ENV_DB_PATH).or_else(|_| std::env::var(ENV_DB_PATH_LEGACY)) {
        return Ok(PathBuf::from(path));
    }

    let cache_dir = dirs::cache_dir()
        .context("Could not find cache directory")?
        .join(CACHE_DIR_NAME);

    // Canonicalize to handle VFS remounts / symlinks pointing to the same project
    let canonical_root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    // Create hash from project root for unique DB per project
    let project_hash = simple_hash(canonical_root.to_string_lossy().as_ref());
    let db_dir = cache_dir.join(&project_hash);

    // Auto-migrate: if new hash dir doesn't have a DB, look for old one
    if !db_dir.join("index.db").exists()
        && let Ok(entries) = std::fs::read_dir(&cache_dir)
    {
        for entry in entries.flatten() {
            let old_dir = entry.path();
            if old_dir.is_dir()
                && old_dir.file_name().map(|n| n.to_string_lossy().to_string())
                    != Some(project_hash.clone())
            {
                let old_db = old_dir.join("index.db");
                if old_db.exists() {
                    // Check if this DB belongs to our project by reading metadata
                    if let Ok(conn) = rusqlite::Connection::open(&old_db) {
                        let root_str: Result<String, _> = conn.query_row(
                            "SELECT value FROM metadata WHERE key = 'project_root'",
                            [],
                            |row| row.get(0),
                        );
                        if let Ok(root_val) = root_str
                            && root_val == project_root.to_string_lossy().as_ref()
                        {
                            // Found old DB for this project — migrate
                            let _ = std::fs::create_dir_all(&db_dir);
                            for suffix in DB_FILE_SUFFIXES {
                                let src = old_dir.join(suffix);
                                if src.exists() {
                                    let _ = std::fs::rename(&src, db_dir.join(suffix));
                                }
                            }
                            let _ = std::fs::remove_dir(&old_dir);
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(db_dir.join("index.db"))
}

/// Deterministic hash (djb2 algorithm) — stable across Rust versions unlike DefaultHasher
fn simple_hash(s: &str) -> String {
    let mut hash: u64 = DJB2_SEED;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    format!("{:x}", hash)
}

/// Remove old kotlin-index cache dir entirely
pub fn cleanup_legacy_cache() {
    if let Some(cache_dir) = dirs::cache_dir() {
        let old_dir = cache_dir.join(LEGACY_CACHE_DIR_NAME);
        if old_dir.exists() {
            let _ = std::fs::remove_dir_all(&old_dir);
        }
    }
}

/// Migrate project DB from old kotlin-index dir to new ast-index dir
pub fn migrate_legacy_project(project_root: &Path) {
    let cache_dir = match dirs::cache_dir() {
        Some(d) => d,
        None => return,
    };
    let project_hash = simple_hash(project_root.to_string_lossy().as_ref());
    let old_db_dir = cache_dir.join(LEGACY_CACHE_DIR_NAME).join(&project_hash);
    let new_db_dir = cache_dir.join(CACHE_DIR_NAME).join(&project_hash);

    if !old_db_dir.exists() || new_db_dir.join("index.db").exists() {
        return;
    }

    let _ = std::fs::create_dir_all(&new_db_dir);
    for suffix in DB_FILE_SUFFIXES {
        let src = old_db_dir.join(suffix);
        if src.exists() {
            let _ = std::fs::rename(&src, new_db_dir.join(suffix));
        }
    }
    // Remove old project dir if empty
    let _ = std::fs::remove_dir(&old_db_dir);
}

/// Acquire an exclusive lock file for rebuild operations.
/// Returns the lock file handle — lock is held until the handle is dropped.
/// If another process holds the lock, returns an error immediately.
pub fn acquire_rebuild_lock(project_root: &Path) -> Result<File> {
    use fs2::FileExt;

    let db_path = get_db_path(project_root)?;
    let lock_path = db_path.with_extension("lock");

    // Ensure parent dir exists
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let lock_file = File::create(&lock_path)?;
    lock_file.try_lock_exclusive()
        .map_err(|_| anyhow::anyhow!("Another rebuild is already running for this project. Wait for it to finish or remove {}", lock_path.display()))?;
    Ok(lock_file)
}

/// Delete DB file and WAL/SHM files for the project
pub fn delete_db(project_root: &Path) -> Result<()> {
    let db_path = get_db_path(project_root)?;
    for suffix in ["", "-wal", "-shm"] {
        let p = db_path.with_extension(format!("db{}", suffix));
        if p.exists() {
            std::fs::remove_file(&p)?;
        }
    }
    Ok(())
}

/// Initialize the database schema
pub fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- Files table
        CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            mtime INTEGER NOT NULL,
            size INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);

        -- Symbols table (classes, interfaces, functions, etc.)
        CREATE TABLE IF NOT EXISTS symbols (
            id INTEGER PRIMARY KEY,
            file_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            line INTEGER NOT NULL,
            parent_id INTEGER,
            signature TEXT,
            FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
        CREATE INDEX IF NOT EXISTS idx_symbols_kind ON symbols(kind);
        CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_id);

        -- FTS5 virtual table for full-text search
        CREATE VIRTUAL TABLE IF NOT EXISTS symbols_fts USING fts5(
            name,
            signature,
            content=symbols,
            content_rowid=id
        );

        -- Triggers to keep FTS in sync
        CREATE TRIGGER IF NOT EXISTS symbols_ai AFTER INSERT ON symbols BEGIN
            INSERT INTO symbols_fts(rowid, name, signature) VALUES (new.id, new.name, new.signature);
        END;
        CREATE TRIGGER IF NOT EXISTS symbols_ad AFTER DELETE ON symbols BEGIN
            INSERT INTO symbols_fts(symbols_fts, rowid, name, signature) VALUES('delete', old.id, old.name, old.signature);
        END;
        CREATE TRIGGER IF NOT EXISTS symbols_au AFTER UPDATE ON symbols BEGIN
            INSERT INTO symbols_fts(symbols_fts, rowid, name, signature) VALUES('delete', old.id, old.name, old.signature);
            INSERT INTO symbols_fts(rowid, name, signature) VALUES (new.id, new.name, new.signature);
        END;

        -- Modules table
        CREATE TABLE IF NOT EXISTS modules (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            path TEXT NOT NULL,
            kind TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_modules_name ON modules(name);

        -- Module dependencies
        CREATE TABLE IF NOT EXISTS module_deps (
            id INTEGER PRIMARY KEY,
            module_id INTEGER NOT NULL,
            dep_module_id INTEGER NOT NULL,
            dep_kind TEXT,
            FOREIGN KEY (module_id) REFERENCES modules(id) ON DELETE CASCADE,
            FOREIGN KEY (dep_module_id) REFERENCES modules(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_module_deps_module ON module_deps(module_id);
        CREATE INDEX IF NOT EXISTS idx_module_deps_dep ON module_deps(dep_module_id);

        -- Inheritance/implementation relationships
        CREATE TABLE IF NOT EXISTS inheritance (
            id INTEGER PRIMARY KEY,
            child_id INTEGER NOT NULL,
            parent_name TEXT NOT NULL,
            kind TEXT NOT NULL,
            FOREIGN KEY (child_id) REFERENCES symbols(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_inheritance_child ON inheritance(child_id);
        CREATE INDEX IF NOT EXISTS idx_inheritance_parent ON inheritance(parent_name);

        -- References table (symbol usages)
        CREATE TABLE IF NOT EXISTS refs (
            id INTEGER PRIMARY KEY,
            file_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            line INTEGER NOT NULL,
            context TEXT,
            FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_refs_name ON refs(name);
        CREATE INDEX IF NOT EXISTS idx_refs_file ON refs(file_id);

        -- XML usages (classes used in XML layouts)
        CREATE TABLE IF NOT EXISTS xml_usages (
            id INTEGER PRIMARY KEY,
            module_id INTEGER,
            file_path TEXT NOT NULL,
            line INTEGER NOT NULL,
            class_name TEXT NOT NULL,
            usage_type TEXT,
            element_id TEXT,
            FOREIGN KEY (module_id) REFERENCES modules(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_xml_usages_class ON xml_usages(class_name);
        CREATE INDEX IF NOT EXISTS idx_xml_usages_module ON xml_usages(module_id);

        -- Resources definitions
        CREATE TABLE IF NOT EXISTS resources (
            id INTEGER PRIMARY KEY,
            module_id INTEGER,
            type TEXT NOT NULL,
            name TEXT NOT NULL,
            file_path TEXT NOT NULL,
            line INTEGER,
            FOREIGN KEY (module_id) REFERENCES modules(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_resources_name ON resources(name);
        CREATE INDEX IF NOT EXISTS idx_resources_type ON resources(type);
        CREATE INDEX IF NOT EXISTS idx_resources_module ON resources(module_id);

        -- Resource usages
        CREATE TABLE IF NOT EXISTS resource_usages (
            id INTEGER PRIMARY KEY,
            resource_id INTEGER,
            usage_file TEXT NOT NULL,
            usage_line INTEGER NOT NULL,
            usage_type TEXT,
            FOREIGN KEY (resource_id) REFERENCES resources(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_resource_usages_resource ON resource_usages(resource_id);

        -- Transitive dependencies cache
        CREATE TABLE IF NOT EXISTS transitive_deps (
            id INTEGER PRIMARY KEY,
            module_id INTEGER NOT NULL,
            dependency_id INTEGER NOT NULL,
            depth INTEGER NOT NULL,
            path TEXT,
            FOREIGN KEY (module_id) REFERENCES modules(id) ON DELETE CASCADE,
            FOREIGN KEY (dependency_id) REFERENCES modules(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_transitive_deps_module ON transitive_deps(module_id);
        CREATE INDEX IF NOT EXISTS idx_transitive_deps_dep ON transitive_deps(dependency_id);

        -- iOS storyboard/xib usages
        CREATE TABLE IF NOT EXISTS storyboard_usages (
            id INTEGER PRIMARY KEY,
            module_id INTEGER,
            file_path TEXT NOT NULL,
            line INTEGER NOT NULL,
            class_name TEXT NOT NULL,
            usage_type TEXT,
            storyboard_id TEXT,
            FOREIGN KEY (module_id) REFERENCES modules(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_storyboard_usages_class ON storyboard_usages(class_name);
        CREATE INDEX IF NOT EXISTS idx_storyboard_usages_module ON storyboard_usages(module_id);

        -- iOS assets (from .xcassets)
        CREATE TABLE IF NOT EXISTS ios_assets (
            id INTEGER PRIMARY KEY,
            module_id INTEGER,
            type TEXT NOT NULL,
            name TEXT NOT NULL,
            file_path TEXT NOT NULL,
            FOREIGN KEY (module_id) REFERENCES modules(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_ios_assets_name ON ios_assets(name);
        CREATE INDEX IF NOT EXISTS idx_ios_assets_type ON ios_assets(type);

        -- iOS asset usages
        CREATE TABLE IF NOT EXISTS ios_asset_usages (
            id INTEGER PRIMARY KEY,
            asset_id INTEGER,
            usage_file TEXT NOT NULL,
            usage_line INTEGER NOT NULL,
            usage_type TEXT,
            FOREIGN KEY (asset_id) REFERENCES ios_assets(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_ios_asset_usages_asset ON ios_asset_usages(asset_id);

        -- Metadata for storing index settings
        CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        "#,
    )?;
    Ok(())
}

/// Open or create database connection
pub fn open_db(project_root: &Path) -> Result<Connection> {
    let db_path = get_db_path(project_root)?;
    let conn = Connection::open(&db_path)?;

    // Enable foreign keys and WAL mode for better performance
    conn.pragma_update(None, "foreign_keys", "ON")?;
    // journal_mode returns result, use query_row
    let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "cache_size", SQLITE_CACHE_SIZE)?; // 8 MB cache to limit memory
    let _: i64 = conn.query_row(
        &format!("PRAGMA busy_timeout = {}", SQLITE_BUSY_TIMEOUT_MS),
        [],
        |row| row.get(0),
    )?; // Wait up to 5s if DB is locked

    // Store project root for hash migration
    conn.execute(
        "CREATE TABLE IF NOT EXISTS metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        [],
    )
    .ok();
    conn.execute(
        "INSERT OR REPLACE INTO metadata (key, value) VALUES ('project_root', ?1)",
        params![project_root.to_string_lossy().as_ref()],
    )
    .ok();

    Ok(conn)
}

/// Check if database exists and is initialized
pub fn db_exists(project_root: &Path) -> bool {
    if let Ok(db_path) = get_db_path(project_root) {
        if !db_path.exists() {
            return false;
        }
        // Also check if tables exist
        if let Ok(conn) = Connection::open(&db_path) {
            conn.query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='files'",
                [],
                |_| Ok(()),
            )
            .is_ok()
        } else {
            false
        }
    } else {
        false
    }
}

/// Open the index DB, or print a warning and return None if the index doesn't exist.
pub fn open_db_or_warn(root: &Path) -> Result<Option<Connection>> {
    if !db_exists(root) {
        println!(
            "{}",
            "Index not found. Run 'ast-index rebuild' first.".red()
        );
        return Ok(None);
    }
    Ok(Some(open_db(root)?))
}

/// Symbol kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Class,
    Interface,
    Object,
    Enum,
    Function,
    Property,
    TypeAlias,
    // Perl-specific
    Package,
    Constant,
    // For imports/includes
    Import,
    // For annotations/decorators
    Annotation,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Class => "class",
            SymbolKind::Interface => "interface",
            SymbolKind::Object => "object",
            SymbolKind::Enum => "enum",
            SymbolKind::Function => "function",
            SymbolKind::Property => "property",
            SymbolKind::TypeAlias => "typealias",
            SymbolKind::Package => "package",
            SymbolKind::Constant => "constant",
            SymbolKind::Import => "import",
            SymbolKind::Annotation => "annotation",
        }
    }
}

/// Insert or update a file record
pub fn upsert_file(conn: &Connection, path: &str, mtime: i64, size: i64) -> Result<i64> {
    conn.execute(
        "INSERT OR REPLACE INTO files (path, mtime, size) VALUES (?1, ?2, ?3)",
        params![path, mtime, size],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Insert a symbol
pub fn insert_symbol(
    conn: &Connection,
    file_id: i64,
    name: &str,
    kind: SymbolKind,
    line: usize,
    signature: Option<&str>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO symbols (file_id, name, kind, line, signature) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![file_id, name, kind.as_str(), line as i64, signature],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Insert inheritance relationship
pub fn insert_inheritance(
    conn: &Connection,
    child_id: i64,
    parent_name: &str,
    kind: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO inheritance (child_id, parent_name, kind) VALUES (?1, ?2, ?3)",
        params![child_id, parent_name, kind],
    )?;
    Ok(())
}

/// Clear all data from the database
pub fn clear_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        DELETE FROM ios_asset_usages;
        DELETE FROM ios_assets;
        DELETE FROM storyboard_usages;
        DELETE FROM resource_usages;
        DELETE FROM resources;
        DELETE FROM xml_usages;
        DELETE FROM transitive_deps;
        DELETE FROM refs;
        DELETE FROM inheritance;
        DELETE FROM module_deps;
        DELETE FROM modules;
        DELETE FROM symbols;
        DELETE FROM files;
        "#,
    )?;
    Ok(())
}

/// Get extra source roots stored in metadata
pub fn get_extra_roots(conn: &Connection) -> Result<Vec<String>> {
    let result: Result<String, _> = conn.query_row(
        "SELECT value FROM metadata WHERE key = 'extra_roots'",
        [],
        |row| row.get(0),
    );
    match result {
        Ok(json) => {
            let roots: Vec<String> = serde_json::from_str(&json).unwrap_or_default();
            Ok(roots)
        }
        Err(_) => Ok(vec![]),
    }
}

/// Add an extra source root
pub fn add_extra_root(conn: &Connection, path: &str) -> Result<()> {
    let mut roots = get_extra_roots(conn)?;
    if !roots.contains(&path.to_string()) {
        roots.push(path.to_string());
    }
    let json = serde_json::to_string(&roots)?;
    conn.execute(
        "INSERT OR REPLACE INTO metadata (key, value) VALUES ('extra_roots', ?1)",
        params![json],
    )?;
    Ok(())
}

/// Remove an extra source root
pub fn remove_extra_root(conn: &Connection, path: &str) -> Result<bool> {
    let mut roots = get_extra_roots(conn)?;
    let len_before = roots.len();
    roots.retain(|r| r != path);
    if roots.len() == len_before {
        return Ok(false);
    }
    let json = serde_json::to_string(&roots)?;
    conn.execute(
        "INSERT OR REPLACE INTO metadata (key, value) VALUES ('extra_roots', ?1)",
        params![json],
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        conn
    }

    #[test]
    fn test_simple_hash_deterministic() {
        let h1 = simple_hash("/Users/test/project");
        let h2 = simple_hash("/Users/test/project");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_simple_hash_different() {
        let h1 = simple_hash("/Users/test/project1");
        let h2 = simple_hash("/Users/test/project2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_init_db() {
        let conn = create_test_db();
        // Check tables exist
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='files'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_escape_fts5_query_simple() {
        assert_eq!(queries::escape_fts5_query("MyClass"), "\"MyClass\"");
    }

    #[test]
    fn test_escape_fts5_query_prefix() {
        assert_eq!(queries::escape_fts5_query("Slow*"), "\"Slow\"*");
        assert_eq!(
            queries::escape_fts5_query("SlowUpstream*"),
            "\"SlowUpstream\"*"
        );
    }

    #[test]
    fn test_escape_fts5_query_empty() {
        assert_eq!(queries::escape_fts5_query(""), "");
        assert_eq!(queries::escape_fts5_query("   "), "");
    }

    #[test]
    fn test_escape_fts5_query_with_quotes() {
        assert_eq!(
            queries::escape_fts5_query("say \"hello\""),
            "\"say \"\"hello\"\"\""
        );
    }

    #[test]
    fn test_upsert_and_search() {
        let conn = create_test_db();
        let file_id = upsert_file(&conn, "src/main.kt", 1000, 100).unwrap();
        assert!(file_id > 0);

        insert_symbol(
            &conn,
            file_id,
            "MyService",
            SymbolKind::Class,
            10,
            Some("class MyService"),
        )
        .unwrap();
        insert_symbol(
            &conn,
            file_id,
            "processData",
            SymbolKind::Function,
            20,
            Some("fun processData()"),
        )
        .unwrap();

        let results = search_symbols(&conn, "MyService", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "MyService");
        assert_eq!(results[0].kind, "class");
        assert_eq!(results[0].path, "src/main.kt");
    }

    #[test]
    fn test_search_empty_query() {
        let conn = create_test_db();
        let results = search_symbols(&conn, "", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_find_files() {
        let conn = create_test_db();
        upsert_file(&conn, "src/main.kt", 1000, 100).unwrap();
        upsert_file(&conn, "src/utils/Helper.kt", 2000, 200).unwrap();

        let files = find_files(&conn, "Helper", 10).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].contains("Helper"));
    }

    #[test]
    fn test_find_symbols_by_name() {
        let conn = create_test_db();
        let file_id = upsert_file(&conn, "src/model.kt", 1000, 100).unwrap();
        insert_symbol(
            &conn,
            file_id,
            "User",
            SymbolKind::Class,
            5,
            Some("data class User"),
        )
        .unwrap();
        insert_symbol(
            &conn,
            file_id,
            "UserRepository",
            SymbolKind::Interface,
            20,
            Some("interface UserRepository"),
        )
        .unwrap();

        let results = find_symbols_by_name(&conn, "User", None, 10).unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.name == "User"));
    }

    #[test]
    fn test_upsert_file_updates_mtime() {
        let conn = create_test_db();
        let _id1 = upsert_file(&conn, "src/main.kt", 1000, 100).unwrap();
        let id2 = upsert_file(&conn, "src/main.kt", 2000, 200).unwrap();
        assert!(
            id2 > 0,
            "upsert should succeed for same path with different mtime"
        );
    }

    #[test]
    fn test_clear_db() {
        let conn = create_test_db();
        let file_id = upsert_file(&conn, "src/main.kt", 1000, 100).unwrap();
        insert_symbol(
            &conn,
            file_id,
            "Test",
            SymbolKind::Class,
            1,
            Some("class Test"),
        )
        .unwrap();

        clear_db(&conn).unwrap();

        let results = search_symbols(&conn, "Test", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_get_stats() {
        let conn = create_test_db();
        let file_id = upsert_file(&conn, "src/main.kt", 1000, 100).unwrap();
        insert_symbol(
            &conn,
            file_id,
            "Foo",
            SymbolKind::Class,
            1,
            Some("class Foo"),
        )
        .unwrap();
        insert_symbol(
            &conn,
            file_id,
            "bar",
            SymbolKind::Function,
            5,
            Some("fun bar()"),
        )
        .unwrap();

        let stats = get_stats(&conn).unwrap();
        assert_eq!(stats.file_count, 1);
        assert_eq!(stats.symbol_count, 2);
    }

    #[test]
    fn test_insert_and_find_inheritance() {
        let conn = create_test_db();
        let file_id = upsert_file(&conn, "src/model.kt", 1000, 100).unwrap();
        insert_symbol(
            &conn,
            file_id,
            "Child",
            SymbolKind::Class,
            1,
            Some("class Child : Parent()"),
        )
        .unwrap();

        let child_id: i64 = conn
            .query_row("SELECT id FROM symbols WHERE name = 'Child'", [], |row| {
                row.get(0)
            })
            .unwrap();
        insert_inheritance(&conn, child_id, "Parent", "extends").unwrap();

        let impls = find_implementations(&conn, "Parent", 10).unwrap();
        assert_eq!(impls.len(), 1);
        assert_eq!(impls[0].name, "Child");
    }

    #[test]
    fn test_count_refs() {
        let conn = create_test_db();
        let count = count_refs(&conn).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_open_db_or_warn_missing_index() {
        // With a non-existent path, open_db_or_warn should return None
        // (and print a warning, but we cannot capture stdout in a unit test)
        let tmp = tempfile::TempDir::new().unwrap();
        let result = open_db_or_warn(tmp.path());
        assert!(
            result.is_ok(),
            "open_db_or_warn should not error on missing DB"
        );
        assert!(
            result.unwrap().is_none(),
            "open_db_or_warn should return None when index does not exist"
        );
    }

    #[test]
    fn test_get_db_path_does_not_create_directory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let fake_project = tmp.path().join("no_mkdir_test_project");
        std::fs::create_dir(&fake_project).unwrap();

        let db_path = get_db_path(&fake_project).unwrap();
        let db_dir = db_path.parent().unwrap();

        assert!(
            !db_dir.exists(),
            "get_db_path must not create the cache directory as a side effect"
        );
    }

    #[test]
    fn test_db_exists_does_not_create_directory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let fake_project = tmp.path().join("no_mkdir_exists_test");
        std::fs::create_dir(&fake_project).unwrap();

        let db_path = get_db_path(&fake_project).unwrap();
        let db_dir = db_path.parent().unwrap();

        assert!(!db_exists(&fake_project));
        assert!(
            !db_dir.exists(),
            "db_exists must not create the cache directory as a side effect"
        );
    }
}
