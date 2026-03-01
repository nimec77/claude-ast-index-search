# Flutter/Dart Support

## Progress Report

### Feature Phases

| Phase | Status | Progress |
|-------|--------|----------|
| 1. Project Detection | ⬜ Not Started | 0/4 |
| 2. Module & Dependency Support | ⬜ Not Started | 0/4 |
| 3. Claude Code Plugin Integration | ⬜ Not Started | 0/4 |
| 4. Testing & Verification | ⬜ Not Started | 0/4 |

**Legend:** ⬜ Not Started | 🔄 In Progress | ✅ Complete | ⏸️ Blocked

**Current Phase:** 1
**Last Updated:** 2026-03-01

---

## Phase 1: Project Detection

**Goal:** `ast-index` correctly identifies Flutter projects and roots.

- [ ] 1.1 Add `Flutter` variant to `ProjectType` enum in `src/indexer.rs`
- [ ] 1.2 Add `pubspec.yaml` detection to `detect_project_type()` in `src/indexer.rs` — return `ProjectType::Flutter` when present
- [ ] 1.3 Update `find_project_root()` in `src/indexer.rs` and `src/main.rs` — walk up to the directory containing `pubspec.yaml`
- [ ] 1.4 Update `has_build_marker()` in `src/indexer.rs` — return `true` for `pubspec.yaml` when `ProjectType` is `Flutter`

**Test:** Running `ast-index index` in a Flutter project root detects `ProjectType::Flutter`.

---

## Phase 2: Module & Dependency Support

**Goal:** Parse `pubspec.yaml` to extract the module name and dependencies.

- [ ] 2.1 Add `pubspec.yaml` to `is_module_file()` in `src/indexer.rs` — treat it as the module descriptor for Flutter projects
- [ ] 2.2 Add `serde_yaml` to `Cargo.toml` and implement YAML parsing to extract `name:` field from `pubspec.yaml` in `index_modules_from_files()`
- [ ] 2.3 Implement `index_module_dependencies()` for Flutter — parse `dependencies:` and `dev_dependencies:` sections from `pubspec.yaml` and write to the `module_deps` table
- [ ] 2.4 Add unit tests for `pubspec.yaml` module name extraction and dependency parsing

**Test:** `ast-index modules` lists the Flutter module name; `ast-index deps` shows packages from `pubspec.yaml`.

---

## Phase 3: Claude Code Plugin Integration

**Goal:** Provide a `initialize-flutter` plugin command with Flutter-specific rules and slash commands.

- [ ] 3.1 Create `plugin/commands/initialize-flutter.md` — document Flutter project setup steps, recommended `CLAUDE.md` snippets, and common `ast-index` invocations for Flutter codebases
- [ ] 3.2 Add Flutter-specific search rules to the plugin command (e.g. widget class detection, provider/bloc pattern hints)
- [ ] 3.3 Add Flutter-specific slash command examples (e.g. `ast-index search --kind Class --lang dart`)
- [ ] 3.4 Verify `plugin/SKILL.md` (or equivalent index) references the new `initialize-flutter` command

**Test:** The `initialize-flutter` command file is present and loadable; content covers Flutter project conventions.

---

## Phase 4: Testing & Verification

**Goal:** Full test coverage and clean build for Flutter/Dart support.

- [ ] 4.1 Add project detection unit tests — assert `detect_project_type()` returns `Flutter` for a directory containing `pubspec.yaml`
- [ ] 4.2 Add module parsing unit tests — fixture `pubspec.yaml` with `name:`, `dependencies:`, and `dev_dependencies:` sections; assert correct extraction
- [ ] 4.3 Add an end-to-end integration test using a minimal Flutter project fixture — index, then query symbols, modules, and deps
- [ ] 4.4 Run `cargo clippy -- -D warnings` and `cargo fmt` — zero new warnings, all formatting clean

**Test:** `cargo test` green across all modules; `cargo clippy -- -D warnings` passes with no new warnings.

---

## Notes

- Each phase builds on previous ones
- Complete all tasks in a phase before moving to next
- Update progress table after completing each phase
- Run `cargo test` after each task to catch regressions
- Key files: `src/indexer.rs`, `src/main.rs`, `Cargo.toml`, `plugin/commands/initialize-flutter.md`
