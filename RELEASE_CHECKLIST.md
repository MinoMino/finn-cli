# Release checklist

Use this checklist when preparing a GitHub release for `finn-cli`.

## Before tagging

- [ ] Confirm the working tree is clean: `git status`
- [ ] Run formatting: `cargo fmt --all`
- [ ] Run tests: `cargo test --all-targets`
- [ ] Review README examples against the current CLI
- [ ] Review category resolution behavior against live FINN pages
- [ ] Confirm `.gitignore` is still correct
- [ ] Update `Cargo.toml` version if needed
- [ ] Update `README.md` if user-facing behavior changed

## Create the release commit/tag

- [ ] Commit release prep changes
- [ ] Create an annotated tag, for example:
  - `git tag -a v0.1.0 -m "Release v0.1.0"`
- [ ] Push branch and tags:
  - `git push origin main --tags`

## GitHub release notes template

Title:

- [ ] `vX.Y.Z`

Summary:

- [ ] Briefly describe what this release adds or changes

Highlights:

- [ ] Search Torget listings from the CLI
- [ ] Human-readable and JSON output
- [ ] Item lookup by FINN id or URL
- [ ] Category lookup by name/path
- [ ] Alias + typo-tolerant category matching
- [ ] Interactive category picker

Verification:

- [ ] CI is green
- [ ] Local `cargo test --all-targets` passed
- [ ] Basic smoke tests completed:
  - [ ] `finn-cli search rtx 4080 --category electronics`
  - [ ] `finn-cli search rtx 4080 --pick-category`
  - [ ] `finn-cli item 451260160`
  - [ ] `finn-cli categories data`

Artifacts:

- [ ] Attach binaries if you build release artifacts
- [ ] Or document install via `cargo install --path .`
