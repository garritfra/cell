# Releasing

Releases are automated with
[release-plz](https://release-plz.dev/) from
[`.github/workflows/release.yml`](.github/workflows/release.yml).

The `release.yml` workflow is configured as the crates.io trusted-publishing
provenance source for both `cell-sheet-core` and `cell-sheet-tui`. Do not
rename or replace it without updating the trusted-publishing configuration on
crates.io for both crates.

## Normal Release Flow

1. Land user-visible changes on `main` with conventional commit messages.
2. The release workflow opens or updates a `release-plz` pull request with:
   - the next workspace version in `Cargo.toml`
   - the matching `CHANGELOG.md` release section
   - any lockfile changes needed for the release
3. Review the release PR and merge it when the changelog and version are right.
4. After the merge, `release-plz` publishes unpublished crates to crates.io via
   trusted publishing, creates a single `vX.Y.Z` tag, and creates a draft
   GitHub Release.
5. The same workflow runs again for the new `vX.Y.Z` tag, builds release
   binaries, uploads archives plus `.sha256` checksum files, and publishes the
   draft GitHub Release.

No manual tag push is needed.

## Workflow Ownership

`release-plz` owns:

- release PR creation
- version and changelog updates
- crates.io publishing
- the `vX.Y.Z` tag
- the draft GitHub Release

The tag-triggered artifact jobs in `release.yml` own:

- Linux, macOS, and Windows binary builds
- release archives
- SHA256 checksum files
- publishing the draft GitHub Release after artifacts are attached

The workspace has two public crates but one product release, so
[`release-plz.toml`](release-plz.toml) disables per-crate tags by default and
enables a single `v{{ version }}` tag/release for `cell-sheet-tui`.

## Failure Handling

- If crates.io publishing fails, fix the issue and rerun the failed workflow.
  Do not create a manual tag for the same version.
- If artifact building fails after crates.io publishing succeeds, the GitHub
  Release remains a draft. Fix the workflow or code, rerun the tag workflow,
  and let it upload artifacts and publish the draft.
- If the release PR has the wrong changelog or version, edit the source commits
  or `release-plz.toml` configuration and let the release PR update.
