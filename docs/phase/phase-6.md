# Phase 6: Large Module Decomposition

**Goal:** Reduce the 5 remaining 1000+ line files by extracting inline test modules to sibling files, splitting `main.rs` into focused modules, extracting Dart's error recovery subsystem, and deduplicating `find_capture` across 14 parser files.

**Constraint:** No behavioral changes. Each task must pass `cargo test && cargo clippy -- -D warnings && cargo fmt --check`.

## Tasks

### 6.1 Extract Parser Tests to Separate Files

Using `#[cfg(test)] #[path = "<lang>_tests.rs"] mod tests;` pattern.

- [x] 6.1.1 Extract `dart.rs` tests (L1095–1860, 766 lines) → `src/parsers/treesitter/dart_tests.rs`
- [x] 6.1.2 Extract `csharp.rs` tests (L518–1431, 914 lines) → `src/parsers/treesitter/csharp_tests.rs`
- [x] 6.1.3 Extract `cpp.rs` tests (L574–1305, 731 lines) → `src/parsers/treesitter/cpp_tests.rs`
- [x] 6.1.4 Extract `typescript.rs` tests (L803–1200, 398 lines) → `src/parsers/treesitter/typescript_tests.rs`

### 6.2 Split `main.rs` (1043 → ~330 lines)

- [x] 6.2.1 Move `Cli` struct, `Commands` enum, and `find_project_root()` to new `src/cli.rs` (~660 lines); add `mod cli;` + `use cli::{Cli, Commands, find_project_root};` in `main.rs`
- [x] 6.2.2 Move `cmd_install_claude_plugin()` to `src/commands/management.rs` as `pub fn`; update dispatch in `main.rs`

### 6.3 Extract Dart Error Recovery Submodule

*Depends on: 6.1.1*

- [x] 6.3.1 Move error recovery structs/functions from `dart.rs` (post-test ~L855–1093) to new `src/parsers/treesitter/dart_error_recovery.rs`; add `#[path = "dart_error_recovery.rs"] mod error_recovery;` in `dart.rs`

### 6.4 Deduplicate `find_capture` Across 14 Parsers

*Depends on: 6.1.1–6.1.4*

- [x] 6.4.1 Add `pub(crate) fn find_capture` to `src/parsers/treesitter/mod.rs`; remove local copies from `cpp.rs`, `csharp.rs`, `typescript.rs`, `go.rs`, `java.rs`, `kotlin.rs`, `objc.rs`, `php.rs`, `proto.rs`, `python.rs`, `ruby.rs`, `rust_lang.rs`, `scala.rs`, `swift.rs`; add `find_capture` to each file's `use super::` import

### 6.5 Documentation Updates

- [x] 6.5.1 Update `CLAUDE.md` architecture section for new file layout
- [x] 6.5.2 Add Phase 6 to `docs/tasklist.md` *(this entry)*

## Acceptance Criteria

**Test:** After all tasks: `cargo fmt --check && cargo clippy -- -D warnings && cargo clippy --tests -- -D warnings && cargo test && cargo test --test memory_tests -- --test-threads=1`

**Expected file size reductions:** `dart.rs` −68%, `csharp.rs` −64%, `cpp.rs` −56%, `typescript.rs` −33%, `main.rs` −68%

## Dependencies

- Phase 5 complete

## Implementation Notes

- Each sub-task should be committed independently after passing all checks
- No behavioral changes — this is purely structural/cleanup work
- 6.3 depends on 6.1.1 (Dart tests must be extracted before moving error recovery code)
- 6.4 depends on 6.1.1–6.1.4 (all test extractions must complete before deduplicating `find_capture`)
- Run `cargo test` after each sub-task to catch regressions early
- Key files: `src/parsers/treesitter/dart.rs`, `src/parsers/treesitter/csharp.rs`, `src/parsers/treesitter/cpp.rs`, `src/parsers/treesitter/typescript.rs`, `src/main.rs`
