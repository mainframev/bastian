---
name: release-recovery
description: Recover bastian from a failed Release workflow run, after a release PR merged and its `v*` tag was pushed. Use when a Release run fails, a version tag has no GitHub release, crates.io or the Homebrew tap lags a tag, or the user links a failing release job.
---

# Recover a failed release

The pipeline (`.github/workflows/`): **Prepare Release** opens `release/vX.Y.Z` → merging it runs **Create Tag**, which pushes `vX.Y.Z` → that tag runs **Release**: `checks` (ci.yml) → `build` (4 targets) → `publish-crates` → `release` (GitHub release) → `homebrew` (tap formula).

The job that failed decides what was already published, so start there.

## 1. Find the failure

```sh
gh run list --workflow Release --limit 5
gh run view <run-id>                  # which jobs passed, failed, were skipped
gh run view <run-id> --log-failed | grep -E 'error|-->' | head -40
gh release view vX.Y.Z                # does the GitHub release exist?
cargo search bastian --limit 1        # is X.Y.Z on crates.io?
```

Reproduce a `checks` failure locally before fixing it. CI uses the latest stable Rust; check yours with `.github/scripts/toolchain-check.sh` first, because an older clippy passes code that CI fails on (v0.4.1 failed that way).

## 2. Decide by the failed job

| Failed job | Already published | Recovery |
|---|---|---|
| `checks`, `build` | nothing | Fix on main, then move the tag (A) or cut the next patch (B) |
| `publish-crates` | maybe the crate | Check crates.io. Not there and the cause is transient: rerun (C). There: never move the tag; finish by hand (D) |
| `release` | the crate | Fix the cause (token, permissions), then rerun (C). `publish-crates` reruns only if it failed |
| `homebrew` | crate and GitHub release | Fix `HOMEBREW_TAP_TOKEN` or the tap, then rerun (C) |

A crates.io version can never be uploaded again, only yanked. Once the crate is out, the tag must stay on the commit it was built from.

## 3. Recovery paths

**A. Move the tag (nothing published, the fix isn't user facing).** Use this when the fix is `test:`, `chore:`, `ci:` or similar, which git-cliff leaves out of the changelog, so `CHANGELOG.md` for X.Y.Z stays correct. Steps:

1. Land the fix on main through a normal PR, after the full local check (`just check`, toolchain current).
2. **Confirm with the user before running these.** They delete and rewrite a published tag:
   ```sh
   git fetch origin --tags && git switch main && git pull --ff-only
   grep -m1 '^version' Cargo.toml        # must still be X.Y.Z
   git push origin :refs/tags/vX.Y.Z && git tag -d vX.Y.Z
   git tag -a vX.Y.Z -m "Release X.Y.Z" && git push origin vX.Y.Z
   ```
   A tag pushed with the user's credentials starts Release. One pushed with `GITHUB_TOKEN` doesn't, which is why Create Tag uses `RELEASE_TOKEN`.
3. Watch it: `gh run watch $(gh run list --workflow Release --limit 1 --json databaseId --jq '.[0].databaseId')`.

**B. Next patch (nothing published, the fix is user facing: `fix:`/`feat:`).** Land the fix, then run Prepare Release (`gh workflow run prepare-release.yml -f bump=patch`) and merge its PR. Leave the failed tag in place, or delete it with the user's OK. Never reuse its version.

**C. Rerun failed jobs (transient: network, runner, an expired secret now fixed).**
```sh
gh run rerun <run-id> --failed
```
Reruns use the workflow file at the tag's commit. If the workflow itself is broken, a fix on main doesn't reach this run: move the tag (A) if nothing is published, otherwise finish by hand (D).

**D. Finish by hand (crate published, a later job can't rerun cleanly).** Get the user's OK, then do only the steps that are missing. Download the build artifacts with `gh run download <run-id> -D artifacts`. Then create the release from the job's commands in `release.yml` (`git cliff --latest --strip header`, `sha256sum`, `gh release create vX.Y.Z ...`). For the tap, the commands are in the `homebrew` job (`.github/scripts/homebrew-formula.sh`).

## Rules

- Deleting or moving a tag, force-pushing, yanking a crate and creating a release by hand are all outward facing: confirm each with the user first.
- Don't `cargo publish` from a local checkout. The workflow publishes from the tag.
- Don't edit `CHANGELOG.md` or the version in `Cargo.toml` by hand. Prepare Release owns them.
- The fix goes through a normal PR with the local checks in `.github/pull_request_template.md`. CI doesn't run on PRs, so a failed release is often the first time CI sees the code.
- Report the end state: tag commit, Release run URL, and whether the crate, the GitHub release and the tap are all at X.Y.Z.
