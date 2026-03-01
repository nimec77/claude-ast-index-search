# Summary: CS-1-1 -- Flutter/Dart Project Detection

**Ticket:** CS-1-1
**Status:** IMPLEMENT_STEP_OK
**Date:** 2026-03-01

---

## What Was Done

Phase 1 of Flutter/Dart support adds project detection to `ast-index`. No Dart source file parsing is included; the scope is limited to correctly identifying Flutter projects and their roots.

Four changes were made across two files:

### 1. `ProjectType::Flutter` enum variant (`src/indexer.rs`)

A new `Flutter` variant was inserted into the `ProjectType` enum between `Bazel` and `Mixed`, and `as_str()` was extended to return `"Flutter (Dart)"` for this variant.

### 2. Flutter detection in `detect_project_type()` (`src/indexer.rs`)

`pubspec.yaml` is checked at the project root. A `has_flutter` boolean is added to the multi-platform count array (enabling `Mixed` detection when `pubspec.yaml` coexists with other markers such as `package.json`). An `else if has_flutter` branch was added before the `Unknown` fallback, returning `ProjectType::Flutter` when the marker is found alone.

### 3. Flutter root resolution in `find_project_root()` (`src/main.rs`)

The ancestor-walking loop in `find_project_root()` was extended to return early when `pubspec.yaml` is found in an ancestor directory. The check is placed after the `.xcodeproj` block and before the Bazel markers block.

### 4. Build marker recognition in `has_build_marker()` (`src/indexer.rs`)

`|| path.join("pubspec.yaml").exists()` was appended to the existing boolean chain. This allows monorepo sub-project detection to recognize Flutter sub-projects as project boundaries.

---

## Decisions Made

### `has_build_marker()` signature kept unchanged (ADR CS-1-1)

The PRD flagged a design ambiguity: `has_build_marker()` takes only `path: &Path` and has no `ProjectType` parameter, so it is not possible to conditionally check `pubspec.yaml` only for Flutter projects. Two options were evaluated:

- **Option A (chosen):** Add `pubspec.yaml` unconditionally, consistent with how `Makefile`, `BUILD`, and `CMakeLists.txt` are already handled. One-line change, no call-site impact.
- **Option B (rejected):** Add a `ProjectType` parameter. More precise but a broader refactor, inconsistent with the existing pattern, and over-engineered for Phase 1.

Decision: Option A. `pubspec.yaml` is specific enough to Dart/Flutter that false positives in non-Flutter monorepos are not a realistic concern.

### `find_project_root()` updated only in `src/main.rs`

Phase-1.md task 1.3 described updating `find_project_root()` in both `src/indexer.rs` and `src/main.rs`. Investigation showed the function exists only in `src/main.rs`. Only that file was modified. The deviation is documented in the plan and tasklist.

### Mixed detection is correct behavior for Flutter Web projects

Flutter Web projects often contain `package.json` for tooling. When both `pubspec.yaml` and `package.json` are present, `detect_project_type()` returns `ProjectType::Mixed` (count > 1). This is intentional and consistent with the existing multi-platform detection architecture. No special handling was added for Phase 1.

---

## Tests Added

Three unit tests were added to `src/indexer.rs`:

- `test_detect_flutter_project` -- creates `pubspec.yaml` alone, asserts `ProjectType::Flutter`
- `test_detect_mixed_flutter_and_frontend` -- creates `pubspec.yaml` + `package.json`, asserts `ProjectType::Mixed`
- `test_has_build_marker_pubspec` -- creates `pubspec.yaml`, asserts `has_build_marker()` returns `true`

---

## Known Gaps (from QA Report)

- `find_project_root()` with the Flutter marker has no automated unit test. Manual end-to-end verification covers this path. A dedicated test is recommended for Phase 2.
- `ProjectType::Flutter.as_str()` returning `"Flutter (Dart)"` is not directly asserted in any test. Low risk for Phase 1.

---

## Files Modified

- `src/indexer.rs` -- enum variant, `as_str()`, `has_build_marker()`, `detect_project_type()`, three new unit tests
- `src/main.rs` -- `find_project_root()` ancestor walk

---

## Scope Boundary

Phase 1 establishes project identification only. Dart source file parsing (indexing classes, functions, etc.) is out of scope and targeted for a subsequent phase.
