# Summary: CS-5 -- Codebase Refactoring (Phase 5)

**Ticket:** CS-5
**Status:** IMPLEMENT_STEP_OK
**Date:** 2026-03-01

---

## What Was Done

CS-5 is a comprehensive structural refactoring of the `ast-index` codebase. The work spans nine sub-tasks (5.1--5.9) covering dead code removal, duplication elimination, and module splitting. No observable behavior was changed. All 399 tests pass (380 unit + 19 memory), clippy reports zero warnings, and formatting is clean.

The implementation is complete in the working tree. All nine tasks are done. As noted in the QA report, the PRD constraint of one commit per sub-task was not met -- the code changes are present but uncommitted.

### Task 5.1 -- Remove Dead Dependencies and Dead-Code Suppression

Removed `grep-matcher = "0.1"` and `parking_lot = "0.12"` from `Cargo.toml`. Both crates were listed as dependencies but never imported anywhere in `src/`. Removed `#![allow(dead_code)]` from the top of `src/db.rs`. Any dead-code warnings surfaced by clippy after this removal were resolved individually.

**Files:** `Cargo.toml`, `src/db.rs`

### Task 5.2 -- Extract "Index Not Found" Guard Helper

Added `pub fn open_db_or_warn(root: &Path) -> Result<Option<Connection>>` to `src/db.rs` (at line 389). This function calls `db_exists(root)`, prints the red "Index not found. Run 'ast-index rebuild' first." warning if the DB is absent, and returns `Ok(None)`. Replaced all 24 copy-pasted guard blocks across 9 command files with the canonical pattern:

```rust
let conn = match db::open_db_or_warn(root)? {
    Some(c) => c,
    None => return Ok(()),
};
```

Call sites: `index.rs` (6), `management.rs` (5), `modules.rs` (4), `android.rs` (2), `ios.rs` (2), `project_info.rs` (2), `files.rs` (1), `watch.rs` (1), `analysis.rs` (1). Two remaining `db::db_exists(root)` calls in `management.rs` are intentional -- they serve different purposes (pre-rebuild and pre-delete checks) and are not guard replacements.

**Files:** `src/db.rs`, all 9 command files under `src/commands/`

### Task 5.3 -- Deduplicate DB Query Functions

Added `SearchResult::from_row(&rusqlite::Row) -> rusqlite::Result<SearchResult>` and `RefResult::from_row(&rusqlite::Row) -> rusqlite::Result<RefResult>` to `src/db/queries.rs`. Replaced 15+ identical row-mapping closures across query functions with calls to these methods.

Renamed `SearchScope::none()` to `SearchScope::empty()` for consistency with `is_empty()` (an idiomatic Rust constructor name). Made each of the four non-scoped functions a thin wrapper delegating to its scoped counterpart:

- `search_symbols` calls `search_symbols_scoped` with `SearchScope::empty()`
- `find_symbols_by_name` calls `find_symbols_by_name_scoped` with `SearchScope::empty()`
- `find_class_like` calls `find_class_like_scoped` with `SearchScope::empty()`
- `find_references` calls `find_references_scoped` with `SearchScope::empty()`

**Files:** `src/db.rs` / `src/db/queries.rs`

### Task 5.4 -- Unify `search_files` / `search_files_limited`

In `src/commands/mod.rs`, replaced the body of `search_files` (which duplicated ~100 lines of WalkBuilder/channel setup) with a single delegation call:

```rust
search_files_limited(root, pattern, extensions, usize::MAX, handler)
```

Fixed the channel bound in `search_files_limited` by using `limit.clamp(1_000, 10_000)` instead of `limit.max(1000)`, preventing an enormous channel allocation when `limit` is `usize::MAX`. This matches the original `search_files` channel size of 10_000 for unlimited callers.

`src/commands/mod.rs` reduced from ~269 lines to 181 lines.

**Files:** `src/commands/mod.rs`

### Task 5.5 -- Extract WalkBuilder Ignore Setup Helper

Added `pub fn configure_walk_ignores(builder: &mut WalkBuilder, arc_root: Option<&Path>)` to `src/indexer.rs` (at line 389). The function adds `.gitignore` and `.arcignore` custom ignore filenames and the root `.gitignore` file when an arc root is present. Replaced 5 duplicated setup blocks:
- `src/indexer.rs`: 3 blocks (in `index_directory_scoped`, `update_directory_incremental`, `index_modules`)
- `src/commands/mod.rs`: 1 block (in `search_files_limited`, after Task 5.4)
- `src/commands/grep.rs`: 1 block (in `cmd_provides`)

**Files:** `src/indexer.rs`, `src/commands/mod.rs`, `src/commands/grep.rs`

### Task 5.6 -- Extract Constants and Thread Pool Helper

Added three module-level constants at the top of `src/indexer.rs`:

```rust
pub const MAX_FILE_SIZE: u64 = 1_000_000;   // Files larger than 1 MB are skipped
pub const PARSE_CHUNK_SIZE: usize = 500;     // Files per rayon parallel chunk
pub const MAX_WALK_DEPTH: usize = 50;        // Maximum directory walk depth
```

All inline magic number usages in `indexer.rs` and its sub-modules replaced with the named constants.

Extracted `fn build_thread_pool() -> Result<rayon::ThreadPool>` to replace two identical thread pool construction blocks (in `files.rs` and `node_modules.rs`). The helper respects the `AST_INDEX_THREADS` environment variable.

**Files:** `src/indexer.rs`, `src/indexer/files.rs`, `src/indexer/node_modules.rs`

### Task 5.7 -- Deduplicate Perl and Grep Command Boilerplate

Extracted a `grep_and_print` helper in `src/commands/perl.rs` (at line 27) with the signature:

```rust
fn grep_and_print(
    root: &Path,
    pattern: &str,
    extensions: &[&str],
    query: Option<&str>,
    limit: usize,
    label: &str,
    truncate_len: usize,
    extra_filter: Option<fn(&str) -> bool>,
) -> Result<()>
```

All 5 Perl command functions (`cmd_perl_exports`, `cmd_perl_subs`, `cmd_perl_pod`, `cmd_perl_tests`, `cmd_perl_imports`) are now ~3-line wrappers. `perl.rs` reduced from 223 to 142 lines.

Applied the same pattern in `src/commands/grep.rs` (helper at line 34) for `cmd_deprecated`, `cmd_suppress`, `cmd_annotations`, and `cmd_deeplinks`. Two-phase multi-line matching functions (`cmd_composables`, `cmd_previews`) were correctly excluded. `grep.rs` reduced from 941 to 888 lines.

**Files:** `src/commands/perl.rs`, `src/commands/grep.rs`

### Task 5.8 -- Split `indexer.rs` into Sub-Modules

Split the 2992-line `src/indexer.rs` into four child modules using the Rust 2024 file-based module system (no `mod.rs`):

| Sub-module | Contents | Lines |
|---|---|---|
| `src/indexer/files.rs` | `index_directory`, `index_directory_scoped`, `update_directory_incremental`, `parse_file` (pub(crate)), `write_batch_to_db` (pub(crate)) | 506 |
| `src/indexer/modules.rs` | `index_modules`, `index_modules_from_files`, `collect_build_files_from_db`, `index_module_dependencies`, `get_module_deps`, `get_module_dependents` | 469 |
| `src/indexer/resources.rs` | `index_xml_usages`, `index_resources`, `build_transitive_deps`, `index_storyboard_usages`, `index_ios_assets`, `index_ios_package_managers` | 935 |
| `src/indexer/node_modules.rs` | `index_node_modules_dts`, `parse_dts_file` (private) | 205 |

The parent `src/indexer.rs` was trimmed to 931 lines (down from 2992) and retains: `mod` declarations, `pub use` re-exports preserving the full public API, `ProjectType`, `ModuleLookup`, shared constants, and shared helpers (`build_thread_pool`, `configure_walk_ignores`, `detect_project_type`, `find_sub_projects`, etc.).

**Files:** `src/indexer.rs` (trimmed), `src/indexer/files.rs`, `src/indexer/modules.rs`, `src/indexer/resources.rs`, `src/indexer/node_modules.rs` (all new)

### Task 5.9 -- Split `db.rs` into Sub-Modules (Optional, Completed)

Split the 1468-line `src/db.rs` query layer into `src/db/queries.rs` using the Rust 2024 file-based module system:

- `src/db/queries.rs` (543 lines): `SearchResult`, `RefResult`, `SearchScope`, `DbStats`, and all `search_*` / `find_*` functions
- `src/db.rs` trimmed to 795 lines: schema constants, `init_db`, connection/path management, `SymbolKind`, insert functions, extra root functions, `pub use queries::*;`

**Files:** `src/db.rs` (trimmed), `src/db/queries.rs` (new)

---

## Decisions Made

### `SearchScope::none()` renamed to `SearchScope::empty()`

The codebase had `SearchScope::none()` but the PRD specified `SearchScope::empty()`. The plan confirmed the rename is the right choice: `is_empty()` already existed as a method, making `empty()` the idiomatic Rust constructor name (consistent with `Vec::new()` + `Vec::is_empty()` style). The rename was made during Task 5.3. No remaining callers use `none()`.

### `limit.clamp(1_000, 10_000)` instead of `limit.min(10_000).max(1_000)`

The plan specified `limit.min(10_000).max(1_000)` to cap the channel bound in `search_files_limited`. The implementation used `limit.clamp(1_000, 10_000)` which is semantically identical and more idiomatic Rust. QA confirmed equivalence.

### Rust 2024 file-based module system over `mod.rs`

Tasks 5.8 and 5.9 explicitly required the Rust 2024 file-based module system (no `mod.rs` files) per the PRD. This conflicts with `docs/conventions.md` which says "Use `mod.rs` for module entry points." The PRD requirement was treated as authoritative. The conventions document was not updated during this ticket (noted as RISK-3 LOW in QA).

### `resources.rs` not further split

The `resources.rs` sub-module reached 935 lines -- nearly as large as the original `indexer.rs` after trimming. The QA report flagged this as RISK-2 MEDIUM. The decision was to stay within CS-5 scope and not split resources further. A follow-on ticket (potential CS-6) can address it if needed.

### Task 5.9 completed despite being optional

The `db.rs` split (Task 5.9) was marked optional in the PRD. It was completed because the separation between query functions and schema/connection management was clean and the benefit (543 lines of query code isolated in `queries.rs`) was clear. All acceptance criteria for the optional task pass.

### PRD function name mismatch (documentation gap)

The PRD and tasklist named the five Perl commands as `cmd_perl_packages`, `cmd_perl_subs`, `cmd_perl_imports`, `cmd_perl_tests`, `cmd_perl_constants`. The actual codebase functions are `cmd_perl_exports`, `cmd_perl_subs`, `cmd_perl_pod`, `cmd_perl_tests`, `cmd_perl_imports`. The implementation is correct (it matches the actual codebase); the PRD contained stale function names. No code change was needed.

### Incremental commits not made (QA RISK-1 HIGH)

The PRD and plan required each sub-task to be independently committed after passing all checks. All nine tasks were implemented without individual commits. As of branch state, all code changes are uncommitted (only documentation commits exist on the branch). The QA report rates this as HIGH severity as a process concern but confirms the code itself is correct. This should be resolved before or during merge.

---

## Metrics Achieved

| Metric | Before | After |
|---|---|---|
| `src/indexer.rs` lines | 2992 | 931 |
| `src/db.rs` lines | 1468 | 795 |
| `src/commands/mod.rs` lines | ~269 | 181 |
| `src/commands/perl.rs` lines | 223 | 142 |
| `src/commands/grep.rs` lines | 941 | 888 |
| Guard blocks (copy-pasted) | 24 | 0 (replaced by helper) |
| Dead crate dependencies | 2 | 0 |
| Row-mapping closures (duplicated) | 15+ | 0 (replaced by `from_row`) |
| Scoped/non-scoped function pairs | 4 duplicate pairs | 4 thin wrappers |
| WalkBuilder ignore setup blocks | 5 duplicated | 0 (replaced by helper) |
| Magic numbers in `indexer.rs` | 3 values inline | 0 (named constants) |
| Thread pool construction blocks | 2 duplicated | 0 (replaced by helper) |
| Net lines removed (estimated) | -- | ~400-500 |

---

## Verification Results

| Command | Exit Code | Notes |
|---|---|---|
| `cargo fmt --check` | 0 | No formatting differences |
| `cargo clippy -- -D warnings` | 0 | Zero warnings |
| `cargo clippy --tests -- -D warnings` | 0 | Zero warnings |
| `cargo test` | 0 | 399 tests passed (380 unit + 19 memory) |
| `cargo test --test memory_tests -- --test-threads=1` | 0 | 19 passed |

---

## Files Created or Modified

**Modified:**
- `Cargo.toml` -- removed `grep-matcher` and `parking_lot` dependencies
- `src/db.rs` -- removed `#![allow(dead_code)]`, added `open_db_or_warn`, trimmed to schema/connection layer
- `src/indexer.rs` -- trimmed to parent module with re-exports, shared types, constants, helpers
- `src/commands/mod.rs` -- `search_files` delegates to `search_files_limited`, channel bound capped
- `src/commands/perl.rs` -- `grep_and_print` helper extracted, 5 commands become thin wrappers
- `src/commands/grep.rs` -- `grep_and_print` helper extracted, 4 commands become thin wrappers
- `src/commands/analysis.rs`, `android.rs`, `files.rs`, `index.rs`, `ios.rs`, `management.rs`, `modules.rs`, `project_info.rs`, `watch.rs` -- 24 guard blocks replaced with `open_db_or_warn`

**Created:**
- `src/db/queries.rs` -- all query types and functions
- `src/indexer/files.rs` -- file indexing and parsing
- `src/indexer/modules.rs` -- module and dependency indexing
- `src/indexer/resources.rs` -- Android/iOS resource indexing
- `src/indexer/node_modules.rs` -- TypeScript `.d.ts` indexing

---

## Follow-Up Items

1. **Incremental commits (HIGH):** Nine separate commits for sub-tasks 5.1--5.9 should be created to satisfy the PRD constraint and enable future bisect.
2. **Missing-DB warning test (LOW):** A unit test for `open_db_or_warn` against a non-existent path would close the only behavioral gap not covered by `cargo test`.
3. **`resources.rs` further split (LOW):** `src/indexer/resources.rs` at 935 lines is a candidate for a future split into Android and iOS sub-modules.
4. **`docs/conventions.md` update (LOW):** Should note that `src/indexer/` and `src/db/` use the Rust 2024 file-based module system to clarify the team's convention going forward.
