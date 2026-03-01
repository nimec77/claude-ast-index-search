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

**Core flow:** `main.rs` (clap CLI) → `commands/` (command handlers) → `db.rs` (SQLite) and `indexer.rs` (parallel file walk + parsing)

### Key modules

- **`src/indexer.rs`** — File discovery (respects `.gitignore`/`.arcignore`), parallel parsing with rayon in chunks of 500 files, writes to SQLite. Auto-detects `ProjectType` (`Android`, `IOS`, `Frontend`, `Go`, `Rust`, `Python`, `Perl`, `Bazel`, `Flutter`, `Mixed`, `Unknown`) from marker files and handles sub-project mode. Files >1 MB are skipped. `ModuleLookup` assigns files to modules via longest-prefix path matching.
- **`src/db.rs`** — SQLite schema, `SymbolKind` enum, DB path (cache dir keyed by project root hash via djb2), `AST_INDEX_DB_PATH` env override. WAL mode + 5 s busy timeout + exclusive file lock (`index.db.lock`) prevent concurrent rebuild conflicts.
- **`src/parsers/`** — Two-tier parser system:
  - `treesitter/` — Tree-sitter AST parsers (primary, one per language), each implements `LanguageParser` trait. Registered in `treesitter/mod.rs::get_treesitter_parser()`.
  - `perl.rs`, `wsdl.rs` — Regex-based parsers for languages without tree-sitter support.
  - `typescript.rs` — Regex-based parser used for Vue/Svelte script block extraction (TypeScript itself uses tree-sitter).
  - `Vue`/`Svelte` files: script blocks extracted then parsed with the regex TypeScript parser.
  - `treesitter/queries/<lang>.scm` — Tree-sitter S-expression query patterns.
- **`src/commands/`** — One file per command group (`grep.rs`, `management.rs`, `index.rs`, `modules.rs`, `files.rs`, `android.rs`, `ios.rs`, `perl.rs`, `analysis.rs`, `project_info.rs`, `watch.rs`). Grep-based commands use ripgrep internals (`grep-searcher`); index-based commands query SQLite directly.

### Project root detection (`find_project_root()`)

Walks ancestor directories looking for, in order: existing index DB → Gradle markers → `Package.swift`/`.xcodeproj` → `pubspec.yaml` → Bazel `WORKSPACE`. Falls back to CWD.

### Sub-project mode

For large monorepos (≥65 K files with 2+ subdirectories containing platform markers), `find_sub_projects()` partitions the tree and indexes each sub-project separately into the shared DB. Controllable via `--sub-projects` flag.

### Adding a new language parser

1. Add tree-sitter crate to `Cargo.toml`
2. Create `src/parsers/treesitter/queries/<lang>.scm`
3. Create `src/parsers/treesitter/<lang>.rs` implementing `LanguageParser`
4. Register in `src/parsers/treesitter/mod.rs::get_treesitter_parser()`
5. Add file extensions in `src/indexer.rs`
6. Add tests (see existing parsers as reference)

### `ParsedSymbol` and `SymbolKind`

Parsers emit `ParsedSymbol { name, kind, line, signature, parents }`. `signature` is the full source line for context. `parents` is `Vec<(parent_name, inherit_kind)>` used to populate the `inheritance` table. Valid `SymbolKind` values: `Class`, `Interface`, `Object`, `Enum`, `Function`, `Property`, `Constant`, `TypeAlias`, `Package`, `Import`, `Annotation`, `Trait`, `Macro`.

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
