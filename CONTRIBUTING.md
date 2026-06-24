# Contributing

## Getting Started

```bash
cargo test --all-targets
cargo clippy --all-targets
cargo fmt --all
```

## Pull Request Process

1. Create a branch from `dev` (naming: `fix/`, `feat/`, `chore/`, `docs/`)
2. Ensure all tests pass and clippy is clean
3. Update `CHANGELOG.md` under `[Unreleased]`
4. Open PR against `dev` with a clear description
5. Squash-merge after approval

## Code Style

- Zero clippy warnings required
- No `unwrap()` or `expect()` — use pattern matching and proper error handling
- Comments in English, code identifiers in English
- Test all new handlers with at least one positive and one negative case
- Mimic existing patterns for consistency

## Project Structure

```
crates/
  rzd-core/   # Core simulation engine (no JS/browser deps)
  rzd-wasm/   # WASM bindings via wasm-bindgen
  rzd-cli/    # CLI runner via clap
data/
  networks/   # Network topology JSON files
ts-host/      # TypeScript host (Vite)
.github/      # CI workflows, issue templates
```

## Commit Messages

Follow conventional commits:
- `fix:` — bug fix
- `feat:` — new feature
- `chore:` — maintenance
- `docs:` — documentation
- `test:` — tests
- `refactor:` — code restructuring
