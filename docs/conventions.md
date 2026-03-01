# Development Conventions

Rules for code generation. Reference [CLAUDE.md](../CLAUDE.md) for architecture, structure, and technical decisions.

---

## Code Style

**DO:**
- Run `cargo fmt` before committing (default settings, no overrides)
- Group imports: external crates → `std` → `crate::` (blank lines between groups)
- Write `//!` module-level doc comments at the top of each file
- Write `///` doc comments for all `pub` items
- Name command handlers `cmd_<name>`, structs `PascalCase`, statics `SCREAMING_SNAKE_CASE`
- Use `mod.rs` for module entry points (e.g., `parsers/mod.rs`, `commands/mod.rs`)
- Explain *why* in comments, not *what*

**DON'T:**
- Add comments that restate the code
- Use non-default `rustfmt` settings

---

## Architecture

**DO:**
- Use tree-sitter parsers as the primary parsing tier; regex parsers only as fallback for languages without tree-sitter support
- Implement `LanguageParser` trait for every tree-sitter parser
- Declare parser singletons as zero-sized `pub struct`s with a `pub static INSTANCE`
- Register every new parser in `get_treesitter_parser()` in `src/parsers/treesitter/mod.rs`
- Place tree-sitter S-expression queries in `src/parsers/treesitter/queries/<lang>.scm`
- Follow the command pattern: `pub fn cmd_<name>(root: &Path, ...) -> Result<()>`
- Auto-detect `ProjectType` from marker files in `src/indexer.rs`

**DON'T:**
- Add async code — the codebase is synchronous by design
- Put business logic directly in `main.rs`; route through `commands/`
- Duplicate extension lists — add new extensions only in `src/indexer.rs`
- Create a regex parser when a tree-sitter grammar exists for the language

---

## Error Handling

**DO:**
- Use `anyhow` throughout (no `thiserror`)
- Propagate errors with `?` and add context via `.context("...")`
- Use `anyhow!()` for ad-hoc errors
- Use `expect()` for `LazyLock` static initialization (compile-time constant patterns)
- Prefer `unwrap_or` / `unwrap_or_default` / `unwrap_or_else` over bare `unwrap()`

**DON'T:**
- Use `thiserror` or define custom error enums
- Use bare `unwrap()` on runtime values that can legitimately fail
- Expose internal error details to users in final output

---

## Parallelism

**DO:**
- Use `rayon` for parallel file parsing (chunks of 500 files)
- Use `crossbeam-channel` for producer/consumer patterns in grep commands
- Use `thread_local!` for tree-sitter `Parser` instances (they are not `Send`)
- Use `Arc` and atomics for shared state across threads

**DON'T:**
- Use `async`/`await` or `tokio` — there is no async runtime
- Block a rayon thread with long I/O outside of the indexed parsing path
- Share a tree-sitter `Parser` across threads directly

---

## Regex

**DO:**
- Declare every `Regex` as a `static LazyLock<Regex>`
- Call `unwrap()` on `LazyLock` initialization (pattern is a compile-time constant)
- Dereference with `&*RE_NAME` when passing to functions expecting `&Regex`

**DON'T:**
- Compile a `Regex` inside a function body or loop
- Use `once_cell::sync::Lazy` (prefer `std::sync::LazyLock`)

---

## Testing

**DO:**
- Place tests in an inline `#[cfg(test)] mod tests` block at the bottom of the file
- Name tests `test_<what_is_being_tested>`
- Use raw strings `r#"..."#` for source-code fixtures
- Test parser functions directly with representative snippets
- Use `criterion` crate for benchmarks in `benches/`

**DON'T:**
- Use a mocking framework — test real parser output against expected symbols
- Test implementation details; test observable behavior (emitted symbols)
- Make filesystem or network calls in unit tests

---

## Security

**DO:**
- Use HTTPS for any external API calls

**DON'T:**
- Log API keys, tokens, or any credentials at any log level
- Commit secrets or `.env` files

---

## Git Workflow

Use Conventional Commits format. Branch naming: `feature/<ticket>-<short-description>`.

**Pre-commit (required):**
```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

**DON'T:**
- Commit code that fails `clippy -- -D warnings`
- Skip the pre-commit checks
- Force-push to `main`

---

## Absolute Prohibitions

1. **No per-call regex compilation** — always use `static LazyLock<Regex>`
2. **No async runtime** — the project is intentionally synchronous
3. **No secrets in logs** — at any level
4. **No `unsafe`** without a documented justification comment
5. **No code duplication** — extract shared helpers when 3+ sites share structure
6. **No silent failures** — always handle or propagate errors
7. **No new extensions** added outside `src/indexer.rs`
