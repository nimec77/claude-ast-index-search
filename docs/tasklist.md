# Flutter/Dart Support & Codebase Refactoring

## Progress Report

### Feature Phases

| Phase | Status | Progress |
|-------|--------|----------|
| 1. Project Detection | ✅ Complete | 4/4 |
| 2. Module & Dependency Support | ✅ Complete | 4/4 |
| 3. Claude Code Plugin Integration | ✅ Complete | 4/4 |
| 4. Testing & Verification | ✅ Complete | 4/4 |
| 5. Codebase Refactoring | ✅ Complete | 9/9 |
| 6. Large Module Decomposition | ⬜ Not Started | 0/10 |

**Legend:** ⬜ Not Started | 🔄 In Progress | ✅ Complete | ⏸️ Blocked

**Current Phase:** 6
**Last Updated:** 2026-03-02

---

## Phase 1: Project Detection

**Goal:** `ast-index` correctly identifies Flutter projects and roots.

- [x] 1.1 Add `Flutter` variant to `ProjectType` enum in `src/indexer.rs`
- [x] 1.2 Add `pubspec.yaml` detection to `detect_project_type()` in `src/indexer.rs` — return `ProjectType::Flutter` when present
- [x] 1.3 Update `find_project_root()` in `src/indexer.rs` and `src/main.rs` — walk up to the directory containing `pubspec.yaml`
- [x] 1.4 Update `has_build_marker()` in `src/indexer.rs` — return `true` for `pubspec.yaml` when `ProjectType` is `Flutter`

**Test:** Running `ast-index index` in a Flutter project root detects `ProjectType::Flutter`.

---

## Phase 2: Module & Dependency Support

**Goal:** Parse `pubspec.yaml` to extract the module name and dependencies.

- [x] 2.1 Add `pubspec.yaml` to `is_module_file()` in `src/indexer.rs` — treat it as the module descriptor for Flutter projects
- [x] 2.2 Add `serde_yaml` to `Cargo.toml` and implement YAML parsing to extract `name:` field from `pubspec.yaml` in `index_modules_from_files()`
- [x] 2.3 Implement `index_module_dependencies()` for Flutter — parse `dependencies:` and `dev_dependencies:` sections from `pubspec.yaml` and write to the `module_deps` table
- [x] 2.4 Add unit tests for `pubspec.yaml` module name extraction and dependency parsing

**Test:** `ast-index modules` lists the Flutter module name; `ast-index deps` shows packages from `pubspec.yaml`.

---

## Phase 3: Claude Code Plugin Integration

**Goal:** Provide a `initialize-flutter` plugin command with Flutter-specific rules and slash commands.

- [x] 3.1 Create `plugin/commands/initialize-flutter.md` — document Flutter project setup steps, recommended `CLAUDE.md` snippets, and common `ast-index` invocations for Flutter codebases
- [x] 3.2 Add Flutter-specific search rules to the plugin command (e.g. widget class detection, provider/bloc pattern hints)
- [x] 3.3 Add Flutter-specific slash command examples (e.g. `ast-index search --kind Class --lang dart`)
- [x] 3.4 Verify `plugin/SKILL.md` (or equivalent index) references the new `initialize-flutter` command

**Test:** The `initialize-flutter` command file is present and loadable; content covers Flutter project conventions.

---

## Phase 4: Testing & Verification

**Goal:** Full test coverage and clean build for Flutter/Dart support.

- [x] 4.1 Add project detection unit tests — assert `detect_project_type()` returns `Flutter` for a directory containing `pubspec.yaml`
- [x] 4.2 Add module parsing unit tests — fixture `pubspec.yaml` with `name:`, `dependencies:`, and `dev_dependencies:` sections; assert correct extraction
- [x] 4.3 Add an end-to-end integration test using a minimal Flutter project fixture — index, then query symbols, modules, and deps
- [x] 4.4 Run `cargo clippy -- -D warnings` and `cargo fmt` — zero new warnings, all formatting clean

**Test:** `cargo test` green across all modules; `cargo clippy -- -D warnings` passes with no new warnings.

---

---

## Phase 5: Codebase Refactoring

**Goal:** Reduce duplication, remove dead code, split large modules, and extract magic values — without changing any external behavior.

**Constraint:** All phases must pass `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` after each step. No behavioral changes.

### 5.1 Remove Dead Dependencies

- [x] Remove `grep-matcher = "0.1"` from `Cargo.toml` (never imported in `src/`)
- [x] Remove `parking_lot = "0.12"` from `Cargo.toml` (never imported in `src/`)
- [x] Remove `#![allow(dead_code)]` from top of `src/db.rs`; fix any clippy warnings that surface

### 5.2 Extract "Index Not Found" Guard Helper

- [x] Add `pub fn open_db_or_warn(root: &Path) -> Result<Option<Connection>>` to `src/db.rs`
- [x] Replace all 24 copy-pasted guard blocks across command files (`index.rs` ×6, `management.rs` ×5, `modules.rs` ×4, `android.rs` ×2, `ios.rs` ×2, `project_info.rs` ×2, `files.rs` ×1, `watch.rs` ×1, `analysis.rs` ×1) with the helper

### 5.3 Deduplicate DB Query Functions

- [x] Add `SearchResult::from_row` and `RefResult::from_row` methods to `src/db.rs`; replace the 15+ identical row-mapping closures
- [x] Merge each scoped/non-scoped function pair by making the non-scoped version a thin wrapper calling the scoped version with `SearchScope::empty()`:
  - `search_symbols` / `search_symbols_scoped`
  - `find_symbols_by_name` / `find_symbols_by_name_scoped`
  - `find_class_like` / `find_class_like_scoped`
  - `find_references` / `find_references_scoped`

### 5.4 Unify `search_files` / `search_files_limited`

- [x] In `src/commands/mod.rs`, make `search_files` call `search_files_limited` with `usize::MAX` and remove the duplicated WalkBuilder/channel body (~100 lines)

### 5.5 Extract WalkBuilder Ignore Setup Helper

- [x] Add `pub fn configure_walk_ignores(builder: &mut WalkBuilder, arc_root: Option<&Path>)` to `src/indexer.rs`
- [x] Replace 5 duplicated arc/gitignore setup blocks in: `src/indexer.rs` (×3), `src/commands/mod.rs` (×1 after 5.4), `src/commands/grep.rs` (×1)

### 5.6 Extract Constants and Thread Pool Helper

- [x] Add module-level constants to `src/indexer.rs`: `MAX_FILE_SIZE: u64 = 1_000_000`, `PARSE_CHUNK_SIZE: usize = 500`, `MAX_WALK_DEPTH: usize = 50`
- [x] Extract `fn build_thread_pool() -> Result<rayon::ThreadPool>` to replace the two identical thread pool construction blocks (`indexer.rs:628-643` and `indexer.rs:2451-2464`)
- [x] Replace all magic number usages with the named constants

### 5.7 Deduplicate Perl and Grep Command Boilerplate

- [x] In `src/commands/perl.rs`: extract a `grep_and_print(root, pattern, extensions, query, limit, label, extra_filter)` helper; reduce all 5 command functions to ~3-line wrappers (~140 lines removed)
- [x] In `src/commands/grep.rs`: apply the same pattern to `cmd_deprecated`, `cmd_suppress`, `cmd_annotations`, and other functions that share the identical search-collect-print structure (~100 lines removed)

### 5.8 Split `indexer.rs` into Sub-Modules

Using Rust 2024 file-based module system (no `mod.rs`): keep `src/indexer.rs` as parent, add child modules under `src/indexer/`:

- [x] Create `src/indexer/files.rs` — `index_directory`, `index_directory_scoped`, `update_directory_incremental`, `parse_file`, `write_batch_to_db`
- [x] Create `src/indexer/modules.rs` — `index_modules`, `index_modules_from_files`, `collect_build_files_from_db`, `index_module_dependencies`
- [x] Create `src/indexer/resources.rs` — `index_xml_usages`, `index_resources`, `build_transitive_deps`, `index_storyboard_usages`, `index_ios_assets`, `index_ios_package_managers`
- [x] Create `src/indexer/node_modules.rs` — `index_node_modules_dts`, `parse_dts_file`
- [x] Trim `src/indexer.rs` to: re-exports, `ProjectType`, `ModuleLookup`, shared constants, shared helpers, `mod` declarations

### 5.9 Split `db.rs` into Sub-Modules (Optional)

Using Rust 2024 file-based module system (no `mod.rs`): keep `src/db.rs` as parent:

- [x] Create `src/db/queries.rs` — all `search_*`, `find_*`, `SearchResult`, `RefResult`, `SearchScope` functions
- [x] Trim `src/db.rs` to: schema, `init_db`, connection/path management, `SymbolKind`, insert functions, re-exports, `mod` declaration

**Test:** After all tasks: `cargo fmt --check && cargo clippy -- -D warnings && cargo clippy --tests -- -D warnings && cargo test && cargo test --test memory_tests -- --test-threads=1`

---

## Phase 6: Large Module Decomposition

**Goal:** Reduce the 5 remaining 1000+ line files by extracting inline test modules to sibling files, splitting `main.rs` into focused modules, extracting Dart's error recovery subsystem, and deduplicating `find_capture` across 14 parser files.

**Constraint:** No behavioral changes. Each task must pass `cargo test && cargo clippy -- -D warnings && cargo fmt --check`.

### 6.1 Extract Parser Tests to Separate Files

Using `#[cfg(test)] #[path = "<lang>_tests.rs"] mod tests;` pattern.

- [ ] 6.1.1 Extract `dart.rs` tests (L1095–1860, 766 lines) → `src/parsers/treesitter/dart_tests.rs`
- [ ] 6.1.2 Extract `csharp.rs` tests (L518–1431, 914 lines) → `src/parsers/treesitter/csharp_tests.rs`
- [ ] 6.1.3 Extract `cpp.rs` tests (L574–1305, 731 lines) → `src/parsers/treesitter/cpp_tests.rs`
- [ ] 6.1.4 Extract `typescript.rs` tests (L803–1200, 398 lines) → `src/parsers/treesitter/typescript_tests.rs`

### 6.2 Split `main.rs` (1043 → ~330 lines)

- [ ] 6.2.1 Move `Cli` struct, `Commands` enum, and `find_project_root()` to new `src/cli.rs` (~660 lines); add `mod cli;` + `use cli::{Cli, Commands, find_project_root};` in `main.rs`
- [ ] 6.2.2 Move `cmd_install_claude_plugin()` to `src/commands/management.rs` as `pub fn`; update dispatch in `main.rs`

### 6.3 Extract Dart Error Recovery Submodule

*Depends on: 6.1.1*

- [ ] 6.3.1 Move error recovery structs/functions from `dart.rs` (post-test ~L855–1093) to new `src/parsers/treesitter/dart_error_recovery.rs`; add `#[path = "dart_error_recovery.rs"] mod error_recovery;` in `dart.rs`

### 6.4 Deduplicate `find_capture` Across 14 Parsers

*Depends on: 6.1.1–6.1.4*

- [ ] 6.4.1 Add `pub(crate) fn find_capture` to `src/parsers/treesitter/mod.rs`; remove local copies from `cpp.rs`, `csharp.rs`, `typescript.rs`, `go.rs`, `java.rs`, `kotlin.rs`, `objc.rs`, `php.rs`, `proto.rs`, `python.rs`, `ruby.rs`, `rust_lang.rs`, `scala.rs`, `swift.rs`; add `find_capture` to each file's `use super::` import

### 6.5 Documentation Updates

- [ ] 6.5.1 Update `CLAUDE.md` architecture section for new file layout
- [ ] 6.5.2 Add Phase 6 to `docs/tasklist.md` *(this entry)*

**Expected file size reductions:** `dart.rs` −68%, `csharp.rs` −64%, `cpp.rs` −56%, `typescript.rs` −33%, `main.rs` −68%

**Test:** After all tasks: `cargo fmt --check && cargo clippy -- -D warnings && cargo clippy --tests -- -D warnings && cargo test && cargo test --test memory_tests -- --test-threads=1`

---

## Notes

- Each phase builds on previous ones
- Complete all tasks in a phase before moving to next
- Update progress table after completing each phase
- Run `cargo test` after each task to catch regressions
- Key files: `src/indexer.rs`, `src/main.rs`, `Cargo.toml`, `plugin/commands/initialize-flutter.md`
