# Summary: CS-2-2 -- YAML Parsing for Flutter Module Name Extraction

**Ticket:** CS-2-2
**Status:** IMPLEMENT_STEP_OK
**Date:** 2026-03-01

---

## What Was Done

Phase 2.2 adds YAML parsing to extract the `name:` field from `pubspec.yaml` files and insert Flutter module names into the `modules` SQLite table. After this change, `ast-index modules` lists Flutter packages alongside Gradle, SPM, and Perl modules.

Two changes were made across one source file and one manifest:

### 1. `serde_yaml` dependency (`Cargo.toml`)

`serde_yaml = "0.9.34-deprecated"` was added to `[dependencies]` immediately after `serde_json = "1"`. This fits naturally into the existing serde ecosystem (`serde` and `serde_json` were already present). The resolved version includes the `-deprecated` suffix, reflecting the archived state of the upstream crate (see Decisions below).

### 2. `pubspec.yaml` branch in `index_modules_from_files()` (`src/indexer.rs`)

A new branch was added to the `for path in files` loop after the Maven/pom.xml block (line 1100). The branch:

1. Matches when `name_str == "pubspec.yaml"` and the file has a parent directory.
2. Computes `module_path` as the parent directory relative to the project root via `strip_prefix`; root-level `pubspec.yaml` produces an empty string path.
3. Reads file content with `fs::read_to_string(path)`.
4. Defines a local `#[derive(serde::Deserialize)] struct PubSpec { name: Option<String> }` and deserializes with `serde_yaml::from_str`.
5. If `name` is `Some` and non-empty, executes `INSERT OR IGNORE INTO modules (name, path)`.
6. Increments `count`.

Malformed YAML, missing `name:` fields, empty name values, and unreadable files are all silently skipped via `if let Ok(...)` guards. No error is propagated and no module row is inserted for these cases.

---

## Decisions Made

### `serde_yaml` v0.9 chosen over `serde_yml` (ADR CS-2-2)

The phase-2 description and PRD explicitly specify `serde_yaml`. Two options were evaluated:

- **Option A (chosen):** `serde_yaml = "0.9"` (dtolnay, archived). Matches the explicit requirement, stable API, zero migration effort. Archived status is a maintenance risk, not a functional one.
- **Option B (rejected):** `serde_yml` (community fork, actively maintained). Drop-in replacement API, but deviates from the explicit requirement and is still pre-1.0.

Decision: Option A. The requirement is authoritative. If the crate is ever yanked or becomes incompatible, the migration path is a one-line `Cargo.toml` change plus import path rename -- the APIs are identical.

### `PubSpec` struct defined locally inside the branch

The `PubSpec` struct is declared inline inside the `pubspec.yaml` branch, minimizing its scope. This follows the precedent of `PERL_PACKAGE_RE` being defined inside the Perl branch. Only the `name` field is declared; all other YAML fields in `pubspec.yaml` (version, description, dependencies, etc.) are ignored by the deserializer.

### Module name from YAML content, not directory name

The module name is taken from the `name:` field in the YAML file, not derived from the directory path. This matches the SPM pattern (names from file content) and differs from the Gradle pattern (names from path segments). This was unambiguous given the PRD constraints.

### `INSERT OR IGNORE` for duplicate name handling

The `modules` table has a `UNIQUE` constraint on `name`. If two `pubspec.yaml` files in a monorepo share the same `name:` value, the second insertion is silently dropped. This is consistent with the behavior of all other module parsers and is an accepted limitation per the PRD. Dart naming conventions make genuine collisions rare.

---

## Tests Added

Eight unit tests were added to `src/indexer.rs`:

| Test name | Scenario covered |
|---|---|
| `test_is_module_file_includes_pubspec_yaml` | CS-2-1 prerequisite: `is_module_file()` returns `true` for `"pubspec.yaml"` |
| `test_pubspec_yaml_basic_name_extraction` | Root-level `pubspec.yaml` with `name: my_app` -> module row `("my_app", "")` |
| `test_pubspec_yaml_nested_path` | `packages/feature_auth/pubspec.yaml` -> row `("feature_auth", "packages/feature_auth")` |
| `test_pubspec_yaml_missing_name_field_is_skipped` | `pubspec.yaml` without `name:` -> count 0, no row inserted |
| `test_pubspec_yaml_malformed_yaml_is_skipped` | `pubspec.yaml` with `name: [unclosed bracket` -> count 0, no panic |
| `test_pubspec_yaml_complex_structure_extracts_only_name` | Full realistic pubspec with many fields -> only `name:` extracted |
| `test_pubspec_yaml_empty_name_is_skipped` | `name: ""` -> count 0, no row inserted |
| `test_pubspec_yaml_no_regression_in_gradle_parsing` | Mixed project with `build.gradle` + `pubspec.yaml` -> both modules indexed correctly |

---

## Known Gaps (from QA Report)

- **No integration test for `ast-index modules` CLI output.** All tests call `index_modules_from_files()` directly. End-to-end CLI flow (full filesystem walk -> `ast-index modules` output) requires manual smoke testing on a real Flutter project.
- **Duplicate `name:` values across packages have no automated test.** The `INSERT OR IGNORE` behavior for collisions is confirmed by code review only.
- **`serde_yaml = "0.9.34-deprecated"` carries technical debt.** Tracked in ADR `docs/adr/CS-2-2.md`. No immediate action required; migration to `serde_yml` is a future option.

---

## Prerequisite

This ticket depends on CS-2-1, which added `|| name == "pubspec.yaml"` to `is_module_file()`. Without that change, `pubspec.yaml` files are never collected during the filesystem walk and the new branch never executes. The QA report confirmed the CS-2-1 change is present in source at line 443 of `src/indexer.rs`.

---

## Files Modified

- `Cargo.toml` -- added `serde_yaml = "0.9.34-deprecated"` to `[dependencies]`
- `src/indexer.rs` -- `pubspec.yaml` branch in `index_modules_from_files()` (lines 1100-1126); eight new unit tests

---

## Scope Boundary

This ticket extracts only the `name:` field from `pubspec.yaml`. Dependency parsing (`dependencies:` and `dev_dependencies:`) is out of scope and targeted for CS-2-3.
