# Development Conventions

Rules for code generation. Reference [vision.md](vision.md) for architecture, structure, and technical decisions.

---

## Code Style

**DO:**
- Run `cargo fmt` before committing
- Use 100 character line limit
- Group imports: `std` → external → internal (blank lines between)
- Write doc comments (`///`) for all `pub` items
- Explain *why* in comments, not *what*

**DON'T:**
- Use `mod.rs` files (use new module system per vision.md §3)
- Add comments that restate the code

---

## Architecture

**DO:**
- Follow hexagonal architecture (vision.md §2)
- Core depends on traits (ports), adapters implement them
- Pass dependencies via generics or trait objects
- Keep `synapse-core` independent of interfaces

**DON'T:**
- Import adapters into core code
- Make HTTP calls outside provider implementations
- Access struct internals across module boundaries

---

## Error Handling

**DO:**
- Use `thiserror` in `synapse-core` (typed errors)
- Use `anyhow` in CLI/Telegram (ergonomic chains)
- Propagate errors with `?` operator
- Write user-facing messages that are actionable

**DON'T:**
- Use `unwrap()` or `expect()` in library code
- Panic on recoverable errors
- Expose internal error details to users

---

## Async & Safety

**DO:**
- Use `tokio` runtime for all async code
- Implement retry with backoff for network errors
- Validate input at system boundaries

**DON'T:**
- Block the async runtime (no `std::thread::sleep`, use `tokio::time::sleep`)
- Use `unsafe` without justification and documentation
- Store secrets in memory longer than necessary

---

## Testing

**DO:**
- Write unit tests for parsing, serialization, logic
- Use hand-rolled `MockProvider` for trait mocking (no `mockall` dependency)
- Name tests: `test_<function>_<scenario>`
- Target 80% coverage for `synapse-core`

**DON'T:**
- Test implementation details, test behavior
- Skip tests for error paths
- Use real API calls in unit tests

---

## Security

**DO:**
- Log at appropriate levels per vision.md §10
- Require `chmod 600` for config files
- Use HTTPS exclusively for external APIs

**DON'T:**
- Log API keys, tokens, or credentials at any level
- Log user message content above DEBUG
- Commit secrets or `.env` files

---

## Git Workflow

Follow vision.md §12 for commit format, branch naming, and pre-commit checks.

**Pre-commit (required):**
```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

**DON'T:**
- Commit code that fails clippy
- Skip the pre-commit checks
- Force push to `master`

---

## Absolute Prohibitions

1. **No `mod.rs`** — use `module.rs` + `module/` directory pattern
2. **No `unwrap()`/`expect()`** in `synapse-core`
3. **No blocking I/O** in async functions
4. **No secrets in logs** at any level
5. **No direct dependencies** from core to adapters
6. **No code duplication** — extract to shared functions
7. **No silent failures** — always handle or propagate errors
