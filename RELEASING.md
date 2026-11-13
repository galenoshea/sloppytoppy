# Releasing

Releases have two outputs:

- crates.io, for `cargo install sloppytoppy --locked`
- GitHub Releases, for prebuilt macOS/Linux binaries and the shell installer

## First crates.io release

crates.io requires the first version to be published by an authenticated owner:

```sh
cargo publish --dry-run --locked
cargo publish --locked
```

After `0.1.0` exists, configure a crates.io trusted publisher with:

- repository owner: `galenoshea`
- repository: `sloppytoppy`
- workflow: `publish.yml`
- environment: `release`

That allows subsequent version tags to publish with short-lived credentials instead of a stored API token.

## Cut a release

1. Update the version in `Cargo.toml` and add the release to `CHANGELOG.md`.
2. Run the local checks:

   ```sh
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-targets --all-features
   cargo publish --dry-run --locked
   dist plan --tag vX.Y.Z
   ```

3. Commit and push the release changes.
4. Tag that exact commit and push the tag:

   ```sh
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

The tag starts both release workflows. `release.yml` builds four native archives, SHA-256 checksums, provenance attestations, and `sloppytoppy-installer.sh`. `publish.yml` publishes the same version to crates.io, or exits successfully when that version is already present.

Do not reuse or move a published version tag. Crates.io versions and GitHub release artifacts are immutable release inputs.
