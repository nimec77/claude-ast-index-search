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

    let results = find_implementations(&conn, "Parent", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Child");
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
