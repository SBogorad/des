# Releasing rzd-des-engine

## Versioning

The workspace follows semantic versioning. Release notes are maintained in `CHANGELOG.md` under `Unreleased` until a release is cut. Coordinate version changes across `rzd-core`, `rzd-wasm`, and `rzd-cli` when their compatibility contract changes.

## Release checklist

1. Confirm `main` contains the approved changes and that `CHANGELOG.md` describes user-visible changes.
2. Run the standard gates:
   ```sh
   cargo fmt --all --check
   cargo clippy --all-targets -- -D warnings
   cargo test --all-targets
   cargo check -p rzd-wasm --target wasm32-unknown-unknown
   ```
3. Build and verify the TypeScript host:
   ```sh
   pnpm --dir ts-host install --frozen-lockfile
   pnpm --dir ts-host run build
   ```
4. Verify deterministic benchmark and replay expectations for changes that touch the engine, event model, scenario generation, snapshots, traces, or exports.
5. Create and push an annotated `vX.Y.Z` tag from the reviewed `main` commit.

## Release artifacts

The tag workflow builds the native CLI and the `rzd-wasm` package, publishes release assets, and includes `SHA256SUMS`. Verify a downloaded asset with:

```sh
sha256sum --check SHA256SUMS
```

GitHub also emits a build provenance attestation for every published file in `dist/`. Verify a downloaded artifact against the repository and release workflow with:

```sh
gh attestation verify <artifact-file> --repo xodapi/des
```

The workflow does not publish crates automatically. Publishing is an explicit maintainer decision after validating package metadata and public API compatibility.

## Rollback

A GitHub release can be marked as a pre-release or superseded by a corrective tag. Do not rewrite published tags. This repository has no automatic production deployment or schema migration step in its release workflow; downstream deployments must have their own tested rollback procedure.
