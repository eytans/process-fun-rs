# Releasing

Releases are published by the [Release workflow](.github/workflows/release.yml).
Pushing a version tag publishes `process-fun-core`, `process-fun-macro` and
`process-fun` to crates.io, in that order, and creates a GitHub release from
the matching CHANGELOG section.

## Cutting a release

1. In a PR, bump the version to `X.Y.Z`:
   - `version` in `process-fun-core/Cargo.toml`, `process-fun-macro/Cargo.toml`
     and `process-fun/Cargo.toml`
   - the `process-fun-core` / `process-fun-macro` dependency versions in
     `process-fun-macro/Cargo.toml` and `process-fun/Cargo.toml`
   - rename `## [Unreleased]` in `CHANGELOG.md` to `## [X.Y.Z] - YYYY-MM-DD`

   `scripts/check-release.sh X.Y.Z` checks all of this locally.
2. Optional: run the Release workflow manually (Actions → Release → Run
   workflow) with version `X.Y.Z`. It runs every check plus a publish dry run
   and publishes nothing.
3. Merge the PR, then tag the merge commit on `main` and push the tag:

   ```sh
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

The workflow refuses to publish if the tag doesn't match the crate versions or
the CHANGELOG, or if formatting, clippy or tests fail. If publishing fails
halfway, re-run the workflow: crates already on crates.io are skipped.

## crates.io authentication (one-time setup)

The workflow supports two options. Trusted publishing is preferred because
no long-lived secret is stored in GitHub.

**Trusted publishing (no secret).** On crates.io, for each of the three
crates, open *Settings → Trusted Publishing* and add a GitHub publisher:

- Repository owner: `eytans`
- Repository name: `process-fun-rs`
- Workflow filename: `release.yml`
- Environment: `crates-io`

**API token (fallback).** Create a token at crates.io → *Account Settings →
API Tokens* with the `publish-update` scope, limited to the three crates. Add
it as a repository secret named `CARGO_REGISTRY_TOKEN`, under *Settings →
Secrets and variables → Actions*, or as a secret of the `crates-io`
environment. When this secret is set, it is used instead of trusted
publishing.

The publish job runs in the `crates-io` GitHub environment. Under *Settings →
Environments* you can add protection rules to it, such as required reviewers
or allowing only `v*` tags.
