# Phase 2: Module & Dependency Support

**Goal:** Parse `pubspec.yaml` to extract the module name and dependencies.

## Tasks

- [ ] 2.1 Add `pubspec.yaml` to `is_module_file()` in `src/indexer.rs` — treat it as the module descriptor for Flutter projects
- [ ] 2.2 Add `serde_yaml` to `Cargo.toml` and implement YAML parsing to extract `name:` field from `pubspec.yaml` in `index_modules_from_files()`
- [ ] 2.3 Implement `index_module_dependencies()` for Flutter — parse `dependencies:` and `dev_dependencies:` sections from `pubspec.yaml` and write to the `module_deps` table
- [ ] 2.4 Add unit tests for `pubspec.yaml` module name extraction and dependency parsing

## Acceptance Criteria

**Test:** `ast-index modules` lists the Flutter module name; `ast-index deps` shows packages from `pubspec.yaml`.

## Dependencies

- Phase 1 complete

## Implementation Notes

Key files to modify:
- `src/indexer.rs` — `is_module_file()`, `index_modules_from_files()`, `index_module_dependencies()`
- `Cargo.toml` — add `serde_yaml` dependency
