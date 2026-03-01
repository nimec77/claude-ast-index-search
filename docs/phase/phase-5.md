# Phase 5: Codebase Refactoring

**Goal:** Reduce duplication, remove dead code, split large modules, and extract magic values — without changing any external behavior.

**Constraint:** All steps must pass `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` after each sub-task. No behavioral changes.

## Tasks

### 5.1 Remove Dead Dependencies

- [ ] Remove `grep-matcher = "0.1"` from `Cargo.toml` (never imported in `src/`)
- [ ] Remove `parking_lot = "0.12"` from `Cargo.toml` (never imported in `src/`)
- [ ] Remove `#![allow(dead_code)]` from top of `src/db.rs`; fix any clippy warnings that surface

### 5.2 Extract "Index Not Found" Guard Helper

- [ ] Add `pub fn open_db_or_warn(root: &Path) -> Result<Option<Connection>>` to `src/db.rs`
- [ ] Replace all 24 copy-pasted guard blocks across command files (`index.rs` ×6, `management.rs` ×5, `modules.rs` ×4, `android.rs` ×2, `ios.rs` ×2, `project_info.rs` ×2, `files.rs` ×1, `watch.rs` ×1, `analysis.rs` ×1) with the helper

### 5.3 Deduplicate DB Query Functions

- [ ] Add `SearchResult::from_row` and `RefResult::from_row` methods to `src/db.rs`; replace the 15+ identical row-mapping closures
- [ ] Merge each scoped/non-scoped function pair by making the non-scoped version a thin wrapper calling the scoped version with `SearchScope::empty()`:
  - `search_symbols` / `search_symbols_scoped`
  - `find_symbols_by_name` / `find_symbols_by_name_scoped`
  - `find_class_like` / `find_class_like_scoped`
  - `find_references` / `find_references_scoped`

### 5.4 Unify `search_files` / `search_files_limited`

- [ ] In `src/commands/mod.rs`, make `search_files` call `search_files_limited` with `usize::MAX` and remove the duplicated WalkBuilder/channel body (~100 lines)

### 5.5 Extract WalkBuilder Ignore Setup Helper

- [ ] Add `pub fn configure_walk_ignores(builder: &mut WalkBuilder, arc_root: Option<&Path>)` to `src/indexer.rs`
- [ ] Replace 5 duplicated arc/gitignore setup blocks in: `src/indexer.rs` (×3), `src/commands/mod.rs` (×1 after 5.4), `src/commands/grep.rs` (×1)

### 5.6 Extract Constants and Thread Pool Helper

- [ ] Add module-level constants to `src/indexer.rs`: `MAX_FILE_SIZE: u64 = 1_000_000`, `PARSE_CHUNK_SIZE: usize = 500`, `MAX_WALK_DEPTH: usize = 50`
- [ ] Extract `fn build_thread_pool() -> Result<rayon::ThreadPool>` to replace the two identical thread pool construction blocks (`indexer.rs:628-643` and `indexer.rs:2451-2464`)
- [ ] Replace all magic number usages with the named constants

### 5.7 Deduplicate Perl and Grep Command Boilerplate

- [ ] In `src/commands/perl.rs`: extract a `grep_and_print(root, pattern, extensions, query, limit, label, extra_filter)` helper; reduce all 5 command functions to ~3-line wrappers (~140 lines removed)
- [ ] In `src/commands/grep.rs`: apply the same pattern to `cmd_deprecated`, `cmd_suppress`, `cmd_annotations`, and other functions that share the identical search-collect-print structure (~100 lines removed)

### 5.8 Split `indexer.rs` into Sub-Modules

Using Rust 2024 file-based module system (no `mod.rs`): keep `src/indexer.rs` as parent, add child modules under `src/indexer/`:

- [ ] Create `src/indexer/files.rs` — `index_directory`, `index_directory_scoped`, `update_directory_incremental`, `parse_file`, `write_batch_to_db`
- [ ] Create `src/indexer/modules.rs` — `index_modules`, `index_modules_from_files`, `collect_build_files_from_db`, `index_module_dependencies`
- [ ] Create `src/indexer/resources.rs` — `index_xml_usages`, `index_resources`, `build_transitive_deps`, `index_storyboard_usages`, `index_ios_assets`, `index_ios_package_managers`
- [ ] Create `src/indexer/node_modules.rs` — `index_node_modules_dts`, `parse_dts_file`
- [ ] Trim `src/indexer.rs` to: re-exports, `ProjectType`, `ModuleLookup`, shared constants, shared helpers, `mod` declarations

### 5.9 Split `db.rs` into Sub-Modules (Optional)

Using Rust 2024 file-based module system (no `mod.rs`): keep `src/db.rs` as parent:

- [ ] Create `src/db/queries.rs` — all `search_*`, `find_*`, `SearchResult`, `RefResult`, `SearchScope` functions
- [ ] Trim `src/db.rs` to: schema, `init_db`, connection/path management, `SymbolKind`, insert functions, re-exports, `mod` declaration

## Acceptance Criteria

**Test:** After all tasks: `cargo fmt --check && cargo clippy -- -D warnings && cargo clippy --tests -- -D warnings && cargo test && cargo test --test memory_tests -- --test-threads=1`

## Dependencies

- Phase 4 complete

## Implementation Notes

- Each sub-task (5.1–5.9) should be committed independently after passing all checks
- No behavioral changes — this is purely structural/cleanup work
- Run `cargo test` after each sub-task to catch regressions early
- Key files: `src/indexer.rs`, `src/db.rs`, `src/commands/mod.rs`, `src/commands/perl.rs`, `src/commands/grep.rs`, `Cargo.toml`
