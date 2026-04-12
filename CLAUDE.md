# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build
cargo build --release        # Binary: target/release/ast-index

# Test
cargo test                                      # All tests
cargo test parsers::treesitter::typescript      # Single module
cargo test test_pubspec_yaml_basic              # Single test by name (substring match)
cargo test --test memory_tests -- --test-threads=1  # Memory regression tests (MUST be single-threaded)

# Lint
cargo clippy -- -D warnings  # Zero new warnings required
cargo clippy --tests -- -D warnings  # Include test code

# Format
cargo fmt
cargo fmt --check            # Verify only (used in CI)

# Benchmark
cargo bench
```

**Useful env vars during development:**
- `AST_INDEX_DB_PATH=./test.db` — override DB location (avoids polluting `~/.cache`)
- `AST_INDEX_THREADS=2` — cap rayon thread count (default: min(8, cpu_count))

## Architecture

`ast-index` is a fast multi-language code search CLI. It parses source files into a SQLite+FTS5 index and serves queries from it.

**Core flow:** `main.rs` (~330 lines, dispatch only) → `commands/` (command handlers) → `db.rs` (SQLite) and `indexer.rs` (parallel file walk + parsing)

### Key modules

- **`src/main.rs`** — Entry point (~340 lines). Dispatch-only: parses CLI args via `Cli::parse()`, detects project root, computes directory scope, and dispatches to the appropriate `commands::*` handler. CLI definition is in `src/cli.rs`.
- **`src/cli.rs`** — CLI definition (~680 lines): `pub struct Cli` (clap `Parser`), `pub enum Commands` (all subcommands with their argument types), and `pub fn find_project_root()` (walks ancestors for index DB/build markers).
- **`src/indexer.rs`** — Parent module: shared types (`ProjectType`, `ModuleLookup`, `WalkResult`, `ParsedFile`), constants (`MAX_FILE_SIZE=1MB`, `PARSE_CHUNK_SIZE=500`, `MAX_WALK_DEPTH=50`), helpers (`configure_walk_ignores`, `build_thread_pool`), and `pub use` re-exports of the full public API. Sub-modules under `src/indexer/`:
  - **`files.rs`** — Directory walk (respects `.gitignore`/`.arcignore`), parallel parsing with rayon in chunks of 500 files, incremental updates, `write_batch_to_db`. Files >1 MB are skipped.
  - **`modules.rs`** — Gradle/Maven/SPM/Flutter/Bazel module discovery and inter-module dependency indexing.
  - **`resources.rs`** — Android XML layout/resource indexing, iOS storyboard/asset/package-manager indexing.
  - **`node_modules.rs`** — TypeScript `.d.ts` declaration file indexing from `node_modules`.
  - Auto-detects `ProjectType` (`Android`, `IOS`, `Frontend`, `Go`, `Rust`, `Python`, `Perl`, `Bazel`, `Flutter`, `Mixed`, `Unknown`) from marker files. `ModuleLookup` assigns files to modules via longest-prefix path matching.
- **`src/db.rs`** — Parent module: SQLite schema, `SymbolKind` enum, `open_db_or_warn` (guard helper used by all command files), DB path (cache dir keyed by project root hash via djb2), `AST_INDEX_DB_PATH` env override, insert functions, `pub use queries::*` re-export. WAL mode + 5 s busy timeout + exclusive file lock (`index.db.lock`) prevent concurrent rebuild conflicts. Sub-modules:
  - **`src/db/queries.rs`** — All query types (`SearchResult`, `RefResult`, `SearchScope`, `DbStats`) and search/find functions. `SearchScope::empty()` is the constructor for no-scope queries.
  - **`src/db/tests.rs`** — Unit tests extracted from `db.rs` using `#[cfg(test)] mod tests;`.
- **`src/parsers/`** — Two-tier parser system:
  - `treesitter/` — Tree-sitter AST parsers (primary, one per language), each implements `LanguageParser` trait. Registered in `treesitter/mod.rs::get_treesitter_parser()`. Shared helper `pub(crate) fn find_capture` is defined once in `treesitter/mod.rs` and imported by all 14 parser files.
  - `perl.rs`, `wsdl.rs` — Regex-based parsers for languages without tree-sitter support.
  - `typescript.rs` — Regex-based parser used for Vue/Svelte script block extraction (TypeScript itself uses tree-sitter).
  - `Vue`/`Svelte` files: script blocks extracted then parsed with the regex TypeScript parser.
  - `treesitter/queries/<lang>.scm` — Tree-sitter S-expression query patterns.
  - **`treesitter/dart_error_recovery.rs`** — Dart error recovery submodule (~240 lines): structs and functions for recovering class/extension-type declarations from tree-sitter ERROR nodes (`try_recover_from_error`, `extract_parents_from_error_text`, helper finders). Loaded as `#[path = "dart_error_recovery.rs"] mod error_recovery;` inside `dart.rs`.
  - **`treesitter/dart_tests.rs`**, **`treesitter/csharp_tests.rs`**, **`treesitter/cpp_tests.rs`**, **`treesitter/typescript_tests.rs`** — Test modules extracted from the corresponding parser files using `#[cfg(test)] #[path = "<lang>_tests.rs"] mod tests;`. Each file starts with `use super::*;` and contains all `#[test]` functions. The same pattern is used for `db/tests.rs`.
- **`src/commands/`** — One file per command group (`grep.rs`, `management.rs`, `index.rs`, `modules.rs`, `files.rs`, `android.rs`, `ios.rs`, `perl.rs`, `analysis.rs`, `project_info.rs`, `watch.rs`). Grep-based commands use ripgrep internals (`grep-searcher`); index-based commands query SQLite directly. `management.rs` includes `pub fn cmd_install_claude_plugin()`.

### Project root detection (`find_project_root()`)

Defined in `src/cli.rs`. Walks ancestor directories looking for, in order: existing index DB → Gradle markers → `Package.swift`/`.xcodeproj` → `pubspec.yaml` → Bazel `WORKSPACE`. Falls back to CWD.

### Sub-project mode

For large monorepos (≥65 K files with 2+ subdirectories containing platform markers), `find_sub_projects()` partitions the tree and indexes each sub-project separately into the shared DB. Controllable via `--sub-projects` flag.

### Extra roots

`add-root <path>` registers additional source directories (stored in the `metadata` table) that are merged into the main index on `rebuild`. Extra roots survive `rebuild` (they are re-read from the DB, not deleted with it). Use `--force` to add a root that overlaps with the project root.

### Plugin directory

`plugin/` is the Claude Code plugin shipped with the binary. `plugin/.claude-plugin/plugin.json` auto-discovers all files in `./commands` and `./skills`.

- **`plugin/commands/initialize-<lang>.md`** — one per language; slash command that sets up `.claude/settings.json` and `.claude/rules/ast-index.md` in the user's project. All initialize commands share the same 5-step structure (Check Prerequisites → settings.json → rules file → rebuild → verify). The rules file embeds a 3-column Command Reference table and a platform-specific 2-column commands table.
- **`plugin/skills/ast-index/SKILL.md`** — skill descriptor with trigger phrases and the supported-projects table. References per-language detail files.
- **`plugin/skills/ast-index/references/<lang>-commands.md`** — exhaustive command reference for each language; linked from SKILL.md.

The plugin version in `plugin.json` must be bumped in sync with Cargo.toml on release.

### Adding a new language parser

**Rust side:**
1. Add tree-sitter crate to `Cargo.toml`
2. Create `src/parsers/treesitter/queries/<lang>.scm`
3. Create `src/parsers/treesitter/<lang>.rs` implementing `LanguageParser`
4. Register in `src/parsers/treesitter/mod.rs::get_treesitter_parser()`
5. Add file extensions in `src/indexer.rs`
6. Add tests inline in `<lang>.rs` (see smaller parsers as reference); if the test module exceeds ~400 lines, extract to `<lang>_tests.rs` using `#[cfg(test)] #[path = "<lang>_tests.rs"] mod tests;` (see `dart.rs`, `csharp.rs`, `cpp.rs`, `typescript.rs`)

**Plugin side (do alongside the Rust changes):**
7. Create `plugin/commands/initialize-<lang>.md` following the pattern of existing initialize commands; verify step should use a language-relevant search term
8. Create `plugin/skills/ast-index/references/<lang>-commands.md` with the exhaustive command list for the new language
9. Add the language to the supported-projects table in `plugin/skills/ast-index/SKILL.md`

### `ParsedSymbol` and `SymbolKind`

Parsers emit `ParsedSymbol { name, kind, line, signature, parents }`. `signature` is the full source line for context. `parents` is `Vec<(parent_name, inherit_kind)>` used to populate the `inheritance` table. Valid `SymbolKind` values: `Class`, `Interface`, `Object`, `Enum`, `Function`, `Property`, `Constant`, `TypeAlias`, `Package`, `Import`, `Annotation`.

### Database

SQLite stored in `~/.cache/ast-index/<project_hash>/index.db`. Schema includes `files`, `symbols` (with FTS5 virtual table `symbols_fts`), `inheritance`, `modules`, `module_deps`, `refs`, `xml_usages`, `resources`, `resource_usages`, `transitive_deps`, `storyboard_usages`, `ios_assets`, `ios_asset_usages`, `metadata`.

The **rebuild path** (`--rebuild modules` / `--rebuild deps`) uses `collect_build_files_from_db()` which queries the `files` table for known module descriptor filenames. New module file types added to `is_module_file()` must also be added to `collect_build_files_from_db()` to work with incremental rebuilds.

### Module file parsing

`index_modules_from_files()` extracts module names from build descriptors (Gradle, Maven `pom.xml`, Flutter `pubspec.yaml` via `serde_yaml_ng`, SPM `Package.swift`, Bazel `BUILD`). `index_module_dependencies()` extracts inter-module dependencies and writes to `module_deps`, using `module_ids_by_path` (path-keyed) alongside `module_ids` (name-keyed) for lookup.

## Code Style

- Comments in English, concise `/// Check if ...` style docstrings
- Test fixtures use raw strings `r#"..."#`
- Extract helper functions when 3+ match arms share structure
- All regex via `std::sync::LazyLock` — never compile per-call
- In-memory SQLite helpers (`make_modules_db()`, `query_modules()`) for unit tests that need a DB; follow this pattern for new DB-touching tests
- Memory regression tests in `tests/memory_tests.rs` use a custom `#[global_allocator]` — they **must** run with `--test-threads=1` since the allocator counter is global
- Many index-based commands accept `--format json` for machine-readable output
