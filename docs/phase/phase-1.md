# Phase 1: Project Detection

**Goal:** `ast-index` correctly identifies Flutter projects and roots.

## Tasks

- [ ] 1.1 Add `Flutter` variant to `ProjectType` enum in `src/indexer.rs`
- [ ] 1.2 Add `pubspec.yaml` detection to `detect_project_type()` in `src/indexer.rs` — return `ProjectType::Flutter` when present
- [ ] 1.3 Update `find_project_root()` in `src/indexer.rs` and `src/main.rs` — walk up to the directory containing `pubspec.yaml`
- [ ] 1.4 Update `has_build_marker()` in `src/indexer.rs` — return `true` for `pubspec.yaml` when `ProjectType` is `Flutter`

## Acceptance Criteria

**Test:** Running `ast-index index` in a Flutter project root detects `ProjectType::Flutter`.

## Dependencies

- None (first phase)

## Implementation Notes

Key files to modify:
- `src/indexer.rs` — `ProjectType` enum, `detect_project_type()`, `find_project_root()`, `has_build_marker()`
- `src/main.rs` — `find_project_root()` if duplicated there
