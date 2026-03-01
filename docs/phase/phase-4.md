# Phase 4: Testing & Verification

**Goal:** Full test coverage and clean build for Flutter/Dart support.

## Tasks

- [x] 4.1 Add project detection unit tests — assert `detect_project_type()` returns `Flutter` for a directory containing `pubspec.yaml`
- [x] 4.2 Add module parsing unit tests — fixture `pubspec.yaml` with `name:`, `dependencies:`, and `dev_dependencies:` sections; assert correct extraction
- [x] 4.3 Add an end-to-end integration test using a minimal Flutter project fixture — index, then query symbols, modules, and deps
- [x] 4.4 Run `cargo clippy -- -D warnings` and `cargo fmt` — zero new warnings, all formatting clean

## Acceptance Criteria

**Test:** `cargo test` green across all modules; `cargo clippy -- -D warnings` passes with no new warnings.

## Dependencies

- Phase 3 complete

## Implementation Notes

Key files to modify:
- `src/indexer.rs` — add unit tests for `detect_project_type()` and `has_build_marker()`
- `src/indexer.rs` (module parsing section) — add fixture-based tests for `pubspec.yaml` parsing
- Integration test fixture: a minimal `pubspec.yaml` + stub Dart files under a temp directory
