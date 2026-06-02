---
name: release-helper
description: Preview and steer Snapforge releases. CI auto-cuts a PATCH release on every push to main; use this to see exactly what the next release will be, or to correctly force a MINOR/MAJOR bump without fighting the automation. User-invoke only.
disable-model-invocation: true
---

# Snapforge release-helper

Releases are **automated**. On push to `main`, `.github/workflows/release.yml`:
1. Reads current version from `crates/snapforge-core/Cargo.toml` (the single source of truth).
2. Computes next = **patch + 1**.
3. Rewrites the version in `crates/snapforge-core/Cargo.toml` + the two `MACOSX_BUNDLE_*` fields in `qt/CMakeLists.txt`, syncs `Cargo.lock`, commits `chore(release): vX`, tags `vX`, pushes.
4. Runs `packaging/macos/build_dmg.sh` (the same script used locally — single source of truth) and uploads `Snapforge-X.dmg` to the GitHub Release.

Commits starting with `chore(release):` or containing `[skip release]` are skipped, so the bot doesn't loop.

**Do not** hand-commit a `chore(release):` or hand-tag — that's CI's job and a manual tag will collide.

## Modes

### `preview` (default, read-only)
Report current version and what CI will publish on the next push to main.
```bash
CUR=$(grep -m1 '^version = ' crates/snapforge-core/Cargo.toml | sed -E 's/version = "([^"]+)".*/\1/')
IFS='.' read -r MA MI PA <<<"$CUR"
echo "current: $CUR"
echo "CI will publish on next push to main: v$MA.$MI.$((PA+1))"
```
Also surface unreleased commits since the last tag so the user sees what's shipping:
```bash
git log "$(git describe --tags --abbrev=0)"..HEAD --oneline
```

### `minor` / `major` (force a non-patch bump)
CI only ever does patch. To ship `1.4.0` or `2.0.0`, set the version so CI's patch+1 lands on the target.

The trick: CI bumps patch+1, so set the **patch one below the target**.
- Want `1.4.0` → set `crates/snapforge-core/Cargo.toml` to `1.3.<huge>`? No — instead set to `1.4.0` here and push with `[skip release]` won't tag. The reliable path is to **set the exact target version locally, commit it as a normal feat/fix, and let the NEXT real change trigger CI** — but that double-bumps.

Because the +1-patch logic makes a clean minor/major non-trivial, this skill's `minor`/`major` mode should:
1. Edit `crates/snapforge-core/Cargo.toml` version to the target (e.g. `1.4.0`), using the same `sed` anchor CI uses (`0,/^version = "[^"]+"/`).
2. Mirror it into `qt/CMakeLists.txt` (`MACOSX_BUNDLE_BUNDLE_VERSION`, `MACOSX_BUNDLE_SHORT_VERSION_STRING`).
3. Run `cargo update -p snapforge-core --precise <target>` to sync `Cargo.lock`.
4. **Stop and report** the staged diff. Tagging/release stays with CI — but warn the user that CI's patch+1 means they must coordinate (e.g. temporarily set patch to `target_patch - 1`, or adjust `release.yml`'s bump step for this cut). Ask the user how they want to reconcile before pushing.

Never push or tag automatically. Confirm the version-bump policy with the user — the +1-patch automation is opinionated and a wrong minor/major cut is painful to unwind.

## Files
- `crates/snapforge-core/Cargo.toml` — version source of truth
- `qt/CMakeLists.txt` — bundle version (kept in lockstep)
- `.github/workflows/release.yml` — the automation
