# Contributing

## Supported toolchain

- Rust 1.96.0 or the current pinned project toolchain
- Node.js 22
- pnpm 10.19.0 for `ts-host` (root `pnpm-lock.yaml` is authoritative); npm for `visualization` (its `package-lock.json` is authoritative)

The project is a deterministic discrete-event simulator. Use synthetic data only. Never commit credentials, production operational data, personal data, private keys, or generated build output.

## Local validation

Run the narrowest relevant checks, and run the full gate before a broad change:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo check -p rzd-wasm --target wasm32-unknown-unknown
pnpm --dir ts-host install --frozen-lockfile
pnpm --dir ts-host run build
```

For deterministic engine, replay, snapshot, trace, scenario, or export changes, include the seed/scenario and describe benchmark or parity impact in the pull request.

## Pull requests

1. Create a focused branch from `main` using `fix/`, `feat/`, `refactor/`, `chore/`, `docs/`, or `test/`.
2. Make the smallest reviewable change and update `CHANGELOG.md` under `[Unreleased]` for user-visible behavior.
3. Run and report the relevant validation commands.
4. Open the pull request against `main` and complete the pull request template.
5. Merge only after required checks and review pass. Do not rewrite published history.

## Code style

- Keep clippy clean for touched code.
- Prefer explicit error handling over panic-based control flow in production paths.
- Comments and documentation are in English unless a domain quotation requires another language; identifiers are in English.
- Add positive and negative coverage for new validation and handler behavior.

## Releases

Releases use semantic versioning and `vX.Y.Z` tags from `main`. See [docs/RELEASING.md](docs/RELEASING.md). The release workflow builds signed-by-hash artifacts but does not publish crates or deploy production automatically.

## Commit messages

Use conventional prefixes: `fix:`, `feat:`, `chore:`, `docs:`, `test:`, or `refactor:`. Keep commits focused and explain compatibility or migration impact where relevant.
