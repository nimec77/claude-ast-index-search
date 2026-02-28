# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build
cargo build --release        # Binary: target/release/ast-index

# Test
cargo test                   # All tests
cargo test parsers::treesitter::typescript  # Single parser module

# Lint
cargo clippy -- -D warnings  # Zero new warnings required

# Format
cargo fmt

# Benchmark
cargo bench
```

## Architecture

`ast-index` is a fast multi-language code search CLI. It parses source files into a SQLite+FTS5 index and serves queries from it.

**Core flow:** `main.rs` (clap CLI) → `commands/` (command handlers) → `db.rs` (SQLite) and `indexer.rs` (parallel file walk + parsing)

### Key modules

- **`src/indexer.rs`** — File discovery (respects `.gitignore`/`.arcignore`), parallel parsing with rayon in chunks of 500 files, writes to SQLite. Auto-detects `ProjectType` (`Android`, `IOS`, `Frontend`, `Go`, `Rust`, `Python`, `Perl`, `Bazel`, `Mixed`, `Unknown`) from marker files and handles sub-project mode.
- **`src/db.rs`** — SQLite schema, `SymbolKind` enum, DB path (cache dir keyed by project root hash), `AST_INDEX_DB_PATH` env override.
- **`src/parsers/`** — Two-tier parser system:
  - `treesitter/` — Tree-sitter AST parsers (primary, one per language), each implements `LanguageParser` trait. Registered in `treesitter/mod.rs::get_treesitter_parser()`.
  - `perl.rs`, `wsdl.rs` — Regex-based parsers for languages without tree-sitter support.
  - `typescript.rs` — Regex-based parser used for Vue/Svelte script block extraction (TypeScript itself uses tree-sitter).
  - `Vue`/`Svelte` files: script blocks extracted then parsed with the regex TypeScript parser (no tree-sitter parser for these).
  - `treesitter/queries/<lang>.scm` — Tree-sitter S-expression query patterns.
- **`src/commands/`** — One file per command group (analysis, files, modules, android, ios, grep, etc.). Grep-based commands use ripgrep internals (`grep-searcher`), index-based commands query SQLite directly.

### Adding a new language parser

1. Add tree-sitter crate to `Cargo.toml`
2. Create `src/parsers/treesitter/queries/<lang>.scm`
3. Create `src/parsers/treesitter/<lang>.rs` implementing `LanguageParser`
4. Register in `src/parsers/treesitter/mod.rs::get_treesitter_parser()`
5. Add file extensions in `src/indexer.rs`
6. Add tests (see existing parsers as reference)

### `ParsedSymbol` and `SymbolKind`

Parsers emit `ParsedSymbol { name, kind, line, signature, parents }`. Valid `SymbolKind` values: `Class`, `Interface`, `Enum`, `Function`, `Property`, `Constant`, `TypeAlias`, `Package`, `Import`, `Annotation`, `Trait`, `Macro`.

### Database

SQLite stored in `~/.cache/ast-index/<project_hash>/index.db`. Schema includes `files`, `symbols` (with FTS5 index), `inheritance`, `modules`, `module_deps`, `refs`, `xml_usages`, `resources`, `resource_usages`, `transitive_deps`, `storyboard_usages`, `ios_assets`, `ios_asset_usages`.

## Code Style

- Comments in English, concise `/// Check if ...` style docstrings
- Test fixtures use raw strings `r#"..."#`
- Extract helper functions when 3+ match arms share structure
- All `LazyLock` regex — never compile per-call
