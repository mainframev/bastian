## What and why

<!-- What changes for users, and why. Link the issue if there is one. -->

## Verification

CI doesn't run on pull requests; it runs when a release is tagged, so a lint
missed here fails the release. Run these on the PR's final commit:

- [ ] `rustc -V` matches the latest stable (`.github/scripts/toolchain-check.sh` passes); an older clippy misses lints CI fails on
- [ ] `just check` passes: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` with and without `--features mermaid`, `cargo test` with and without `--features mermaid`
- [ ] Snapshot changes (`tests/snapshots/`) were read and are intended, or there are none
- [ ] `just size` fits `.github/size-budget` when the change adds a dependency or a lot of code
- [ ] Every commit is a scoped Conventional Commit (`feat(reader): ...`)
- [ ] A bug fix comes with a regression test that fails without the fix
- [ ] A new or changed key is in the handler, the `HELP` table and `docs/`; a change users see updates `docs/`
