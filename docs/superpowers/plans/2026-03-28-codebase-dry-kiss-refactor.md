# Codebase DRY/KISS Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate code duplication, extract magic strings/numbers into constants, and break oversized functions into focused helpers across the entire codebase.

**Architecture:** This is a pure refactoring effort — no behavioral changes. Each task targets one specific DRY/KISS violation. Tasks are ordered so earlier tasks create shared infrastructure that later tasks consume. Every task must leave `cargo test` and `cargo clippy -- -D warnings` green.

**Tech Stack:** Rust, SQLite (rusqlite), tree-sitter, clap

---

## Phase 1: Shared Infrastructure (commands/common.rs, db constants, parser helpers)

These tasks create the shared modules that all later tasks depend on.

### Task 1: Create `src/commands/common.rs` with timing guard and DB open macro

**Files:**
- Create: `src/commands/common.rs`
- Modify: `src/commands.rs:13-23`

- [ ] **Step 1: Write the test for `CommandTimer`**

Add to the bottom of `src/commands/common.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn command_timer_formats_elapsed() {
        let timer = CommandTimer::new();
        std::thread::sleep(Duration::from_millis(10));
        // Just verify it doesn't panic on format
        let msg = format!("Time: {:?}", timer.start.elapsed());
        assert!(msg.contains("Time:"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test commands::common::tests::command_timer_formats_elapsed -- --nocapture`
Expected: FAIL — module doesn't exist yet

- [ ] **Step 3: Create `src/commands/common.rs`**

```rust
//! Shared helpers for command implementations.

use std::time::Instant;

use colored::Colorize;

/// RAII guard that prints elapsed time on drop.
///
/// Usage: `let _t = CommandTimer::new();`
/// Prints: `Time: 1.23ms` to stderr when dropped.
pub struct CommandTimer {
    pub start: Instant,
}

impl CommandTimer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Drop for CommandTimer {
    fn drop(&mut self) {
        eprintln!("\n{}", format!("Time: {:?}", self.start.elapsed()).dimmed());
    }
}

/// Open the database, printing a warning and returning `Ok(None)` if no index exists.
/// Use with: `let conn = open_db_or_return!(root);`
#[macro_export]
macro_rules! open_db_or_return {
    ($root:expr) => {
        match $crate::db::open_db_or_warn($root)? {
            Some(c) => c,
            None => return Ok(()),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn command_timer_formats_elapsed() {
        let timer = CommandTimer::new();
        std::thread::sleep(Duration::from_millis(10));
        let msg = format!("Time: {:?}", timer.start.elapsed());
        assert!(msg.contains("Time:"));
    }
}
```

- [ ] **Step 4: Register module in `src/commands.rs`**

Add after line 23 (`pub mod watch;`):

```rust
pub mod common;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test commands::common -- --nocapture`
Expected: PASS

- [ ] **Step 6: Run full test suite and clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: All pass, zero warnings

- [ ] **Step 7: Commit**

```bash
git add src/commands/common.rs src/commands.rs
git commit -m "refactor: add commands/common.rs with CommandTimer and open_db_or_return macro"
```

---

### Task 2: Extract DB constants into `src/db.rs`

**Files:**
- Modify: `src/db.rs:1-120`

- [ ] **Step 1: Add constants block at the top of `src/db.rs`**

Insert after the imports (after line 9), before `get_db_path`:

```rust
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
```

- [ ] **Step 2: Replace hardcoded values in `get_db_path()` (lines 12-73)**

Replace `std::env::var("AST_INDEX_DB_PATH").or_else(|_| std::env::var("KOTLIN_INDEX_DB_PATH"))` with:
```rust
std::env::var(ENV_DB_PATH).or_else(|_| std::env::var(ENV_DB_PATH_LEGACY))
```

Replace `.join("ast-index")` with:
```rust
.join(CACHE_DIR_NAME)
```

Replace `for suffix in ["index.db", "index.db-wal", "index.db-shm"]` (line 56) with:
```rust
for suffix in DB_FILE_SUFFIXES
```

- [ ] **Step 3: Replace hardcoded values in `simple_hash()` (line 77)**

Replace `let mut hash: u64 = 5381;` with:
```rust
let mut hash: u64 = DJB2_SEED;
```

- [ ] **Step 4: Replace hardcoded values in `cleanup_legacy_cache()` (line 87)**

Replace `.join("kotlin-index")` with:
```rust
.join(LEGACY_CACHE_DIR_NAME)
```

- [ ] **Step 5: Replace hardcoded values in `migrate_legacy_project()` (lines 101-102)**

Replace `cache_dir.join("kotlin-index")` with `cache_dir.join(LEGACY_CACHE_DIR_NAME)`.
Replace `cache_dir.join("ast-index")` with `cache_dir.join(CACHE_DIR_NAME)`.
Replace the `for suffix in [...]` on line 109 with `for suffix in DB_FILE_SUFFIXES`.

- [ ] **Step 6: Replace hardcoded pragma values**

Find the `cache_size` pragma (currently `"-8000"`) and replace with `SQLITE_CACHE_SIZE`.
Find the `busy_timeout` pragma (currently `5000`) and replace with `SQLITE_BUSY_TIMEOUT_MS`.

- [ ] **Step 7: Run full test suite and clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: All pass, zero warnings

- [ ] **Step 8: Commit**

```bash
git add src/db.rs
git commit -m "refactor: extract magic strings/numbers in db.rs into named constants"
```

---

### Task 3: Extract class-like kinds constant and scoped-query helper in `queries.rs`

**Files:**
- Modify: `src/db/queries.rs`

- [ ] **Step 1: Add the `CLASS_LIKE_KINDS` constant and param helper**

At the top of `queries.rs` (after imports), add:

```rust
/// Symbol kinds that represent class-like declarations (used in `class` and similar queries).
const CLASS_LIKE_KINDS: &[&str] = &[
    "class", "interface", "object", "enum", "protocol", "struct", "actor", "package",
];

/// Build a SQL `IN (...)` clause for class-like kinds.
/// Returns e.g. `('class', 'interface', 'object', ...)`.
fn class_like_in_clause() -> String {
    let items: Vec<String> = CLASS_LIKE_KINDS.iter().map(|k| format!("'{}'", k)).collect();
    format!("({})", items.join(", "))
}

/// Build a parameter vector from initial params + scope params + trailing params,
/// and return both the boxed params and a reference slice suitable for rusqlite.
fn build_query_params(
    initial: Vec<String>,
    scope_params: &[String],
    trailing: Vec<String>,
) -> (Vec<Box<dyn rusqlite::types::ToSql>>, Vec<&dyn rusqlite::types::ToSql>) {
    let mut all: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    for p in initial {
        all.push(Box::new(p));
    }
    for p in scope_params {
        all.push(Box::new(p.clone()));
    }
    for p in trailing {
        all.push(Box::new(p));
    }
    // We need to return refs that borrow from the owned vec. The caller must keep
    // both alive. A helper that returns (owned, refs) won't work because refs borrow owned.
    // Instead, return just the owned vec and let callers do the ref conversion.
    // Actually, let's just provide a function that does the ref conversion:
    let refs: Vec<&dyn rusqlite::types::ToSql> = all.iter().map(|p| p.as_ref()).collect();
    (all, refs)
}
```

Wait — the borrow-checker won't allow returning refs that borrow from `all` in the same tuple. Instead, provide a simpler helper:

```rust
/// Collect boxed query params from: initial values, scope params, and a trailing limit.
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

/// Convert a `Vec<Box<dyn ToSql>>` into a reference slice for rusqlite.
fn params_as_refs(params: &[Box<dyn rusqlite::types::ToSql>]) -> Vec<&dyn rusqlite::types::ToSql> {
    params.iter().map(|p| p.as_ref()).collect()
}
```

- [ ] **Step 2: Replace duplicated class-like IN clause in `find_class_like_scoped()` (line 482)**

Replace the hardcoded `s.kind IN ('class', ...)` string with:
```rust
let kinds_clause = class_like_in_clause();
let sql = format!(
    r#"
    SELECT s.name, s.kind, s.line, s.signature, f.path
    FROM symbols s
    JOIN files f ON s.file_id = f.id
    WHERE s.name = ?1 AND s.kind IN {}{}
    LIMIT ?{}
    "#,
    kinds_clause,
    scope_clause,
    2 + scope_params.len()
);
```

- [ ] **Step 3: Replace duplicated class-like IN clause in `find_class_like_pattern()` (line 560)**

Replace the hardcoded `s.kind IN ('class', ...)` string with:
```rust
let kinds_clause = class_like_in_clause();
let sql = format!(
    r#"
    SELECT s.name, s.kind, s.line, s.signature, f.path
    FROM symbols s
    JOIN files f ON s.file_id = f.id
    WHERE s.name LIKE ?1 AND s.kind IN {}
    ORDER BY length(s.name)
    LIMIT ?2
    "#,
    kinds_clause,
);
```

- [ ] **Step 4: Replace duplicated param building in `search_symbols_scoped()` (lines 373-381)**

Replace:
```rust
let mut all_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
all_params.push(Box::new(escaped_query));
for p in &scope_params {
    all_params.push(Box::new(p.clone()));
}
all_params.push(Box::new(limit as i64));

let param_refs: Vec<&dyn rusqlite::types::ToSql> =
    all_params.iter().map(|p| p.as_ref()).collect();
```

With:
```rust
let all_params = collect_query_params(&[&escaped_query], &scope_params, limit);
let param_refs = params_as_refs(&all_params);
```

- [ ] **Step 5: Apply `collect_query_params` + `params_as_refs` in `find_class_like_scoped()` (lines 490-498)**

Same replacement pattern.

- [ ] **Step 6: Apply `collect_query_params` + `params_as_refs` in `find_references_scoped()` (lines 594-602)**

Same replacement pattern — initial param is `name`.

- [ ] **Step 7: Consolidate `get_stats()` into a single query (lines 122-153)**

Replace 8 separate `SELECT COUNT(*)` calls with one query:

```rust
pub fn get_stats(conn: &Connection) -> Result<DbStats> {
    let sql = r#"
        SELECT
            (SELECT COUNT(*) FROM files),
            (SELECT COUNT(*) FROM symbols),
            (SELECT COUNT(*) FROM modules),
            COALESCE((SELECT COUNT(*) FROM refs), 0),
            COALESCE((SELECT COUNT(*) FROM xml_usages), 0),
            COALESCE((SELECT COUNT(*) FROM resources), 0),
            COALESCE((SELECT COUNT(*) FROM storyboard_usages), 0),
            COALESCE((SELECT COUNT(*) FROM ios_assets), 0)
    "#;
    conn.query_row(sql, [], |row| {
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
    })
    .map_err(Into::into)
}
```

- [ ] **Step 8: Run full test suite and clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: All pass, zero warnings

- [ ] **Step 9: Commit**

```bash
git add src/db/queries.rs
git commit -m "refactor: extract CLASS_LIKE_KINDS constant, query param helpers, and consolidate get_stats into single query"
```

---

### Task 4: Extract capture-index helper in `src/parsers/treesitter.rs`

**Files:**
- Modify: `src/parsers/treesitter.rs`

- [ ] **Step 1: Add `CaptureIndexer` to `treesitter.rs`**

Add after the `find_capture` function (after line 101):

```rust
/// Maps tree-sitter capture names to their numeric indices.
///
/// Every parser recreates this lookup closure. This struct provides
/// the same functionality once.
///
/// Usage:
/// ```ignore
/// let idx = CaptureIndexer::new(query);
/// let idx_class = idx.get("class_name");
/// // then: find_capture(m, idx_class)
/// ```
pub(crate) struct CaptureIndexer {
    names: Vec<String>,
}

impl CaptureIndexer {
    pub fn new(query: &Query) -> Self {
        Self {
            names: query.capture_names().iter().map(|n| n.to_string()).collect(),
        }
    }

    /// Look up the index for a capture name. Returns `None` if the name
    /// is not present in the query (which `find_capture` handles gracefully).
    pub fn get(&self, name: &str) -> Option<u32> {
        self.names
            .iter()
            .position(|n| n == name)
            .map(|i| i as u32)
    }
}
```

- [ ] **Step 2: Run tests to verify nothing breaks**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS (new code is additive, no callers yet)

- [ ] **Step 3: Commit**

```bash
git add src/parsers/treesitter.rs
git commit -m "refactor: add CaptureIndexer helper to eliminate duplicated closure in parsers"
```

---

## Phase 2: Apply Shared Infrastructure Across Commands

### Task 5: Replace timing + DB boilerplate in `src/commands/analysis.rs`

**Files:**
- Modify: `src/commands/analysis.rs`

- [ ] **Step 1: Read the file and identify all `Instant::now()` + `elapsed()` pairs and `open_db_or_warn` patterns**

- [ ] **Step 2: Replace each `Instant::now()` + manual `eprintln!(... elapsed ...)` pair with `CommandTimer`**

At the top, add:
```rust
use super::common::CommandTimer;
```

Replace patterns like:
```rust
let start = Instant::now();
// ... body ...
eprintln!("\n{}", format!("Time: {:?}", start.elapsed()).dimmed());
```
With:
```rust
let _timer = CommandTimer::new();
// ... body (remove the eprintln line) ...
```

- [ ] **Step 3: Replace each `db::open_db_or_warn` match with the macro**

Replace:
```rust
let conn = match db::open_db_or_warn(root)? {
    Some(c) => c,
    None => return Ok(()),
};
```
With:
```rust
let conn = open_db_or_return!(root);
```

Remove `use crate::db;` if it was only used for `open_db_or_warn` (keep if used elsewhere). The macro uses `$crate::db` internally.

- [ ] **Step 4: Remove unused `use std::time::Instant;` if all uses were replaced**

- [ ] **Step 5: Run tests and clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/commands/analysis.rs
git commit -m "refactor: use CommandTimer and open_db_or_return! in analysis.rs"
```

---

### Task 6: Replace timing + DB boilerplate in remaining command files

**Files:**
- Modify: `src/commands/android.rs`
- Modify: `src/commands/ios.rs`
- Modify: `src/commands/modules.rs`
- Modify: `src/commands/files.rs`
- Modify: `src/commands/grep.rs`
- Modify: `src/commands/index.rs`
- Modify: `src/commands/management.rs`
- Modify: `src/commands/perl.rs`
- Modify: `src/commands/project_info.rs`
- Modify: `src/commands/watch.rs`

Apply the same mechanical transformation as Task 5 to every command file:

- [ ] **Step 1: `android.rs` — replace timing + DB open patterns**

Add `use super::common::CommandTimer;`, replace `Instant::now()` + elapsed print with `CommandTimer::new()`, replace `open_db_or_warn` match with `open_db_or_return!`.

- [ ] **Step 2: `ios.rs` — same replacements**

- [ ] **Step 3: `modules.rs` — same replacements**

- [ ] **Step 4: `files.rs` — same replacements**

- [ ] **Step 5: `grep.rs` — same replacements**

Note: `grep.rs` has the `grep_and_print` helper (line 42-72) that uses `Instant::now()` + elapsed. Replace it there too.

- [ ] **Step 6: `index.rs` — same replacements**

- [ ] **Step 7: `management.rs` — same replacements**

- [ ] **Step 8: `perl.rs` — same replacements**

- [ ] **Step 9: `project_info.rs` — same replacements**

- [ ] **Step 10: `watch.rs` — same replacements**

- [ ] **Step 11: Run full test suite and clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: All pass, zero warnings

- [ ] **Step 12: Commit**

```bash
git add src/commands/
git commit -m "refactor: use CommandTimer and open_db_or_return! across all command files"
```

---

### Task 7: Deduplicate SearchScope construction in `main.rs`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Extract a helper function in `main.rs`**

Add before `fn main()`:

```rust
/// Build a `SearchScope` from the common CLI options.
fn make_scope<'a>(
    in_file: Option<&'a str>,
    module: Option<&'a str>,
    dir_prefix: Option<&'a str>,
) -> db::SearchScope<'a> {
    db::SearchScope {
        in_file,
        module,
        dir_prefix,
    }
}
```

- [ ] **Step 2: Replace all 5 inline `SearchScope` constructions**

Replace each occurrence of:
```rust
let scope = db::SearchScope {
    in_file: in_file.as_deref(),
    module: module.as_deref(),
    dir_prefix: dir_prefix_ref,
};
```

With:
```rust
let scope = make_scope(in_file.as_deref(), module.as_deref(), dir_prefix_ref);
```

Occurrences at: lines ~111-115, ~130-134, ~157-161, ~178-182, ~195-199.

- [ ] **Step 3: Run full test suite and clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "refactor: extract make_scope() helper to deduplicate SearchScope construction"
```

---

### Task 8: Deduplicate excluded directories between `index.rs` and `watch.rs`

**Files:**
- Modify: `src/commands/common.rs`
- Modify: `src/commands/index.rs`
- Modify: `src/commands/watch.rs`

- [ ] **Step 1: Read `src/commands/index.rs` and `src/commands/watch.rs` to find the duplicated excluded directory lists**

- [ ] **Step 2: Add the constant to `src/commands/common.rs`**

```rust
/// Directories excluded from sub-project scanning and file watching.
pub const EXCLUDED_SCAN_DIRS: &[&str] = &[
    "build", "node_modules", ".gradle", ".git", "target", ".idea", "__pycache__", ".dart_tool",
];
```

(Use the exact values from the source files — they should be identical.)

- [ ] **Step 3: Replace the hardcoded arrays in `index.rs` and `watch.rs` with `common::EXCLUDED_SCAN_DIRS`**

In both files, replace the local array with:
```rust
use super::common::EXCLUDED_SCAN_DIRS;
```
And use `EXCLUDED_SCAN_DIRS` where the local constant was used.

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/commands/common.rs src/commands/index.rs src/commands/watch.rs
git commit -m "refactor: deduplicate EXCLUDED_SCAN_DIRS between index.rs and watch.rs"
```

---

## Phase 3: Apply CaptureIndexer Across All Tree-Sitter Parsers

### Task 9: Migrate `go.rs` parser to use `CaptureIndexer`

**Files:**
- Modify: `src/parsers/treesitter/go.rs`

- [ ] **Step 1: Replace the inline closure with `CaptureIndexer`**

Replace:
```rust
let capture_names = query.capture_names();
let idx = |name: &str| -> Option<u32> {
    capture_names
        .iter()
        .position(|n| *n == name)
        .map(|i| i as u32)
};
```

With:
```rust
let idx = CaptureIndexer::new(query);
```

Update import line to include `CaptureIndexer`:
```rust
use super::{CaptureIndexer, LanguageParser, find_capture, line_text, node_line, node_text, parse_tree};
```

- [ ] **Step 2: Replace all `idx("capture_name")` calls with `idx.get("capture_name")`**

Change every `let idx_foo = idx("foo");` to `let idx_foo = idx.get("foo");`.

- [ ] **Step 3: Run go parser tests**

Run: `cargo test parsers::treesitter::go`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/parsers/treesitter/go.rs
git commit -m "refactor: use CaptureIndexer in Go parser"
```

---

### Task 10: Migrate remaining 13 tree-sitter parsers to `CaptureIndexer`

**Files:**
- Modify: `src/parsers/treesitter/java.rs`
- Modify: `src/parsers/treesitter/kotlin.rs`
- Modify: `src/parsers/treesitter/swift.rs`
- Modify: `src/parsers/treesitter/python.rs`
- Modify: `src/parsers/treesitter/ruby.rs`
- Modify: `src/parsers/treesitter/rust_lang.rs`
- Modify: `src/parsers/treesitter/scala.rs`
- Modify: `src/parsers/treesitter/cpp.rs`
- Modify: `src/parsers/treesitter/csharp.rs`
- Modify: `src/parsers/treesitter/objc.rs`
- Modify: `src/parsers/treesitter/php.rs`
- Modify: `src/parsers/treesitter/proto.rs`
- Modify: `src/parsers/treesitter/dart.rs`
- Modify: `src/parsers/treesitter/typescript.rs`

Apply the **identical mechanical transformation** from Task 9 to each file:

1. Add `CaptureIndexer` to the `use super::` import
2. Replace the `capture_names`/closure block with `let idx = CaptureIndexer::new(query);`
3. Replace all `idx("name")` calls with `idx.get("name")`

- [ ] **Step 1: `java.rs`** — replace closure with CaptureIndexer
- [ ] **Step 2: `kotlin.rs`** — same
- [ ] **Step 3: `swift.rs`** — same
- [ ] **Step 4: `python.rs`** — same
- [ ] **Step 5: `ruby.rs`** — same (note: ruby has multiple `parse_symbols`-like methods; check for all closures)
- [ ] **Step 6: `rust_lang.rs`** — same
- [ ] **Step 7: `scala.rs`** — same
- [ ] **Step 8: `cpp.rs`** — same
- [ ] **Step 9: `csharp.rs`** — same
- [ ] **Step 10: `objc.rs`** — same
- [ ] **Step 11: `php.rs`** — same
- [ ] **Step 12: `proto.rs`** — same
- [ ] **Step 13: `dart.rs`** — same (dart uses `walk_node` pattern; check if closure appears there)
- [ ] **Step 14: `typescript.rs`** — same

- [ ] **Step 15: Run full parser test suite**

Run: `cargo test parsers::treesitter`
Expected: All PASS

- [ ] **Step 16: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Zero warnings

- [ ] **Step 17: Commit**

```bash
git add src/parsers/treesitter/
git commit -m "refactor: use CaptureIndexer across all 14 tree-sitter parsers"
```

---

## Phase 4: Extract Default Limit Constants in CLI

### Task 11: Extract default limit constants in `src/cli.rs`

**Files:**
- Modify: `src/cli.rs`

- [ ] **Step 1: Add constants at the top of `cli.rs` (after imports)**

```rust
/// Default result limit for most commands
const DEFAULT_LIMIT: &str = "50";

/// Default result limit for commands that return fewer results (class, symbol, etc.)
const DEFAULT_LIMIT_SMALL: &str = "20";

/// Default call-tree depth
const DEFAULT_CALL_DEPTH: &str = "3";

/// Default per-directory limit for map command
const DEFAULT_PER_DIR: &str = "5";

/// Default todo search pattern
const DEFAULT_TODO_PATTERN: &str = "TODO|FIXME|HACK";
```

- [ ] **Step 2: Replace all `default_value = "50"` with `default_value = DEFAULT_LIMIT`**

Search for all `default_value = "50"` in cli.rs and replace with `default_value = DEFAULT_LIMIT`.

- [ ] **Step 3: Replace all `default_value = "20"` with `default_value = DEFAULT_LIMIT_SMALL`**

- [ ] **Step 4: Replace `default_value = "3"` (call-tree depth) with `default_value = DEFAULT_CALL_DEPTH`**

- [ ] **Step 5: Replace `default_value = "5"` (per_dir) with `default_value = DEFAULT_PER_DIR`**

- [ ] **Step 6: Replace `"TODO|FIXME|HACK"` default pattern with `default_value = DEFAULT_TODO_PATTERN`**

- [ ] **Step 7: Run full test suite and clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: All pass

- [ ] **Step 8: Commit**

```bash
git add src/cli.rs
git commit -m "refactor: extract default limit constants in cli.rs"
```

---

## Phase 5: Fix OS-specific path separator

### Task 12: Use `std::path::MAIN_SEPARATOR` in `main.rs`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Replace hardcoded `'/'` with `std::path::MAIN_SEPARATOR`**

In the `dir_prefix` computation (around line 22), replace:
```rust
if !s.ends_with('/') {
    s.push('/');
}
```
With:
```rust
if !s.ends_with(std::path::MAIN_SEPARATOR) {
    s.push(std::path::MAIN_SEPARATOR);
}
```

- [ ] **Step 2: Run tests and clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "refactor: use std::path::MAIN_SEPARATOR instead of hardcoded '/'"
```

---

## Phase 6: Consolidate Indexer Progress Constants

### Task 13: Extract progress-reporting thresholds in indexer

**Files:**
- Modify: `src/indexer.rs` (or top of relevant sub-module)
- Modify: `src/indexer/files.rs`
- Modify: `src/indexer/node_modules.rs`

- [ ] **Step 1: Add constants to `src/indexer.rs`**

Add near the existing constants (`MAX_FILE_SIZE`, `PARSE_CHUNK_SIZE`, `MAX_WALK_DEPTH`):

```rust
/// Log progress every N entries during directory walk
pub const WALK_PROGRESS_INTERVAL: usize = 10_000;

/// Log progress every N files during parsing
pub const PARSE_PROGRESS_INTERVAL: usize = 2_000;

/// Log progress every N files during .d.ts parsing
pub const DTS_PROGRESS_INTERVAL: usize = 1_000;

/// Log progress every N files during incremental update
pub const INCREMENTAL_PROGRESS_INTERVAL: usize = 500;

/// Max depth for node_modules scanning (pnpm/nested packages)
pub const NODE_MODULES_MAX_DEPTH: usize = 8;
```

- [ ] **Step 2: Replace hardcoded values in `files.rs`**

Replace `is_multiple_of(10000)` with `is_multiple_of(WALK_PROGRESS_INTERVAL)`.
Replace `is_multiple_of(2000)` with `is_multiple_of(PARSE_PROGRESS_INTERVAL)`.
Replace `is_multiple_of(500)` with `is_multiple_of(INCREMENTAL_PROGRESS_INTERVAL)`.

- [ ] **Step 3: Replace hardcoded values in `node_modules.rs`**

Replace `is_multiple_of(1000)` with `is_multiple_of(DTS_PROGRESS_INTERVAL)`.
Replace the hardcoded max depth `8` with `NODE_MODULES_MAX_DEPTH`.

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/indexer.rs src/indexer/files.rs src/indexer/node_modules.rs
git commit -m "refactor: extract progress interval and depth constants in indexer"
```

---

## Phase 7: Break Up Large Functions

### Task 14: Split `get_stats()` consolidation — already done in Task 3

This task was completed as part of Task 3 (single-query consolidation). No additional work needed.

---

### Task 15: Extract `find_project_root()` marker detection into data-driven loop

**Files:**
- Modify: `src/cli.rs`

- [ ] **Step 1: Read `find_project_root()` in `cli.rs` to get exact current code**

- [ ] **Step 2: Refactor the marker detection to use a data-driven approach**

Replace the sequential `if ancestor.join("settings.gradle").exists()` / `if ancestor.join("settings.gradle.kts").exists()` / etc. block with:

```rust
/// Project root markers: if any of these files exist in a directory, it's a project root.
const PROJECT_ROOT_MARKERS: &[&str] = &[
    "settings.gradle",
    "settings.gradle.kts",
    "Package.swift",
    "pubspec.yaml",
    "WORKSPACE",
    "WORKSPACE.bazel",
    "MODULE.bazel",
];
```

Then in the function body, replace the individual checks with:

```rust
// Check file-based markers
for marker in PROJECT_ROOT_MARKERS {
    if ancestor.join(marker).exists() {
        return Ok(ancestor.to_path_buf());
    }
}

// Check for .xcodeproj directory (special case: directory, not file)
if let Ok(entries) = std::fs::read_dir(ancestor) {
    if entries
        .flatten()
        .any(|e| e.path().extension().is_some_and(|ext| ext == "xcodeproj"))
    {
        return Ok(ancestor.to_path_buf());
    }
}
```

- [ ] **Step 3: Run tests and clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/cli.rs
git commit -m "refactor: data-driven project root marker detection in find_project_root()"
```

---

## Post-Refactor Verification

### Task 16: Full verification pass

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 2: Run clippy with all checks**

Run: `cargo clippy --tests -- -D warnings`
Expected: Zero warnings

- [ ] **Step 3: Run formatter check**

Run: `cargo fmt --check`
Expected: No formatting issues

- [ ] **Step 4: Build release to ensure no codegen issues**

Run: `cargo build --release`
Expected: Successful build

- [ ] **Step 5: Run memory regression tests**

Run: `cargo test --test memory_tests -- --test-threads=1`
Expected: All pass
