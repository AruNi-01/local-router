# Releasing LocalRouter

This document describes the release process for LocalRouter as it exists in this repository today.

It covers:

- bumping the version
- creating and pushing a Git tag
- letting GitHub Actions build and publish release assets
- verifying the published assets
- validating the install flow
- updating a Homebrew tap

Chinese version:

- [RELEASING.zh-CN.md](./RELEASING.zh-CN.md)

## Release Inputs

Current release plumbing depends on these files:

- [apps/localrouter-cli/Cargo.toml](./apps/localrouter-cli/Cargo.toml)
- [apps/localrouterd/Cargo.toml](./apps/localrouterd/Cargo.toml)
- [scripts/release/build-release.sh](./scripts/release/build-release.sh)
- [scripts/release/local-release.sh](./scripts/release/local-release.sh)
- [scripts/install/install.sh](./scripts/install/install.sh)
- [scripts/release/generate-homebrew-formula.sh](./scripts/release/generate-homebrew-formula.sh)
- [scripts/release/generate-homebrew-tap-readme.sh](./scripts/release/generate-homebrew-tap-readme.sh)
- [scripts/release/update-homebrew-tap.sh](./scripts/release/update-homebrew-tap.sh)
- [.github/release.yml](./.github/release.yml)
- [.github/workflows/release.yml](./.github/workflows/release.yml)

The install script currently points to this GitHub repo:

```text
https://github.com/AruNi-01/local-router
```

## Release Checklist

Before tagging, verify all of the following:

- version is updated
- release notes or changelog material is ready
- `README.md` and `README.zh-CN.md` still reflect the current install flow
- dashboard build still works
- daemon still serves the embedded dashboard
- `localrouter dev` still works end to end

## 1. Bump Versions

At minimum, update the version in:

- [apps/localrouter-cli/Cargo.toml](./apps/localrouter-cli/Cargo.toml)

Recommended:

- keep [apps/localrouterd/Cargo.toml](./apps/localrouterd/Cargo.toml) aligned
- keep [crates/localrouter-core/Cargo.toml](./crates/localrouter-core/Cargo.toml) aligned

Use a plain semver like `0.1.0`. The Git tag must be prefixed with `v`, for example `v0.1.0`.

## 2. Run Preflight Checks

From the repo root:

```bash
cargo fmt --all --check
cargo check
cargo test -p localrouter-core
cd apps/dashboard && npm run build && cd ../..
```

Recommended product-path verification:

```bash
cargo run -p localrouter-cli -- daemon stop
cargo run -p localrouter-cli -- dev --no-open
curl -I http://127.0.0.1:9731/
curl -I http://127.0.0.1:9731/v1/health
cargo run -p localrouter-cli -- daemon stop
```

Optional local release-archive smoke test for your current machine:

Fastest local release path:

```bash
./scripts/release/local-release.sh
```

This builds the current platform release archive and stages a local release directory in:

- `dist/local-release`

It also prints a ready-to-run local install test command using `file://` URLs.

Apple Silicon macOS:

```bash
./scripts/release/build-release.sh aarch64-apple-darwin darwin-arm64
```

Linux x64:

```bash
./scripts/release/build-release.sh x86_64-unknown-linux-gnu linux-x64
```

This should produce:

- `dist/localrouter-v<version>-<platform>.tar.gz`
- `dist/localrouter-v<version>-<platform>.tar.gz.sha256`

## 3. Commit Release Changes

Commit the version bump and any release-related docs or fixes before tagging.

Example:

```bash
git status
git add .
git commit -m "chore: release v0.1.0"
```

## 4. Create and Push the Tag

Create an annotated tag:

```bash
git tag -a v0.1.0 -m "v0.1.0"
git push origin main
git push origin v0.1.0
```

Important:

- the workflow publishes only when the ref is a tag matching `v*`
- `workflow_dispatch` runs the build matrix, but the publish job is still gated on a tag

## 5. Watch the GitHub Actions Workflow

After pushing the tag, open:

- GitHub repo `Actions`
- workflow: `release`

Expected jobs:

- `build`
- `publish`

Expected build matrix platforms:

- `linux-x64`
- `linux-arm64`
- `darwin-x64`
- `darwin-arm64`

The build job:

- builds the dashboard
- builds `localrouter` and `localrouterd`
- packages them into a versioned tarball
- writes a `.sha256` file
- uploads artifacts

The publish job:

- downloads all build artifacts
- flattens them into `release/`
- creates `latest` aliases
- generates `localrouter.rb`
- generates a tap `README.md`
- creates or updates the GitHub Release assets
- asks GitHub to generate release notes automatically
- updates the Homebrew tap automatically when tap automation is configured
- writes a compact release summary to the workflow run summary

## 6. Verify Release Assets

Open the GitHub Release for the tag and verify these assets exist.

Versioned assets:

- `localrouter-v0.1.0-linux-x64.tar.gz`
- `localrouter-v0.1.0-linux-x64.tar.gz.sha256`
- `localrouter-v0.1.0-linux-arm64.tar.gz`
- `localrouter-v0.1.0-linux-arm64.tar.gz.sha256`
- `localrouter-v0.1.0-darwin-x64.tar.gz`
- `localrouter-v0.1.0-darwin-x64.tar.gz.sha256`
- `localrouter-v0.1.0-darwin-arm64.tar.gz`
- `localrouter-v0.1.0-darwin-arm64.tar.gz.sha256`

Latest aliases:

- `localrouter-latest-linux-x64.tar.gz`
- `localrouter-latest-linux-x64.tar.gz.sha256`
- `localrouter-latest-linux-arm64.tar.gz`
- `localrouter-latest-linux-arm64.tar.gz.sha256`
- `localrouter-latest-darwin-x64.tar.gz`
- `localrouter-latest-darwin-x64.tar.gz.sha256`
- `localrouter-latest-darwin-arm64.tar.gz`
- `localrouter-latest-darwin-arm64.tar.gz.sha256`

Homebrew formula:

- `localrouter.rb`
- `homebrew-tap-README.md`

Workflow summary:

- the publish job summary should include the tag, release URL, release ID, asset list, and tap update status

If any of these are missing, stop and fix the workflow before telling users to install the release.

## 7. Validate the No-Source Install Flow

Test the install script against the tagged version:

```bash
LOCALROUTER_VERSION=v0.1.0 \
LOCALROUTER_INSTALL_DIR="$(mktemp -d)" \
bash scripts/install/install.sh
```

Then test the installed binaries:

```bash
"$LOCALROUTER_INSTALL_DIR/localrouter" daemon status
```

Recommended real-world check in a clean shell:

```bash
curl -fsSL https://raw.githubusercontent.com/AruNi-01/local-router/main/scripts/install/install.sh | sh
localrouter dev
```

What to verify:

- archive downloads successfully
- checksum validation passes
- both `localrouter` and `localrouterd` are installed
- `localrouter dev` starts the daemon
- `http://127.0.0.1:9731/` loads the embedded dashboard

Local staged-release validation without GitHub:

```bash
./scripts/release/local-release.sh
INSTALL_DIR="$(mktemp -d)"
LOCALROUTER_BASE_URL="file://$PWD/dist/local-release" \
LOCALROUTER_INSTALL_DIR="$INSTALL_DIR" \
bash scripts/install/install.sh
"$INSTALL_DIR/localrouter" daemon status
```

## 8. Update the Homebrew Tap

The release workflow can update a Homebrew tap automatically after the GitHub Release assets are published.

Automation entry points:

- [scripts/release/generate-homebrew-formula.sh](./scripts/release/generate-homebrew-formula.sh)
- [scripts/release/generate-homebrew-tap-readme.sh](./scripts/release/generate-homebrew-tap-readme.sh)
- [scripts/release/update-homebrew-tap.sh](./scripts/release/update-homebrew-tap.sh)

Required GitHub configuration:

- repo variable: `HOMEBREW_TAP_REPO`
  example: `AruNi-01/homebrew-tap`
- repo secret: `HOMEBREW_TAP_TOKEN`
  this token must be able to push to the tap repo

Optional GitHub variables:

- `HOMEBREW_TAP_BRANCH`
  default: `main`
- `HOMEBREW_TAP_FORMULA_PATH`
  default: `Formula/localrouter.rb`
- `HOMEBREW_TAP_FORMULA_NAME`
  default behavior: `localrouter`
- `HOMEBREW_TAP_README_PATH`
  default: `README.md`
- `HOMEBREW_TAP_UPDATE_README`
  default: `true`
- `HOMEBREW_TAP_GIT_NAME`
  default: `localrouter-bot`
- `HOMEBREW_TAP_GIT_EMAIL`
  default: `localrouter-bot@users.noreply.github.com`

What the workflow does:

1. generates `localrouter.rb`
2. generates a tap `README.md`
3. publishes the GitHub Release assets
4. clones the configured tap repo
5. overwrites the configured formula path and optional README path
6. commits and pushes only if something changed

If `HOMEBREW_TAP_REPO` or `HOMEBREW_TAP_TOKEN` is missing, the tap update step exits successfully and is skipped.

GitHub auto-generated release notes:

- release notes are generated by GitHub automatically during publish
- categories are configured in [.github/release.yml](./.github/release.yml)
- use labels consistently if you want release notes grouped cleanly

Manual fallback:

1. download `localrouter.rb` from the GitHub Release assets
2. clone your tap repo
3. replace `Formula/localrouter.rb`
4. commit and push

Validation after automation:

```bash
brew update
brew install <your-org-or-user>/tap/localrouter
localrouter daemon status
```

## 9. Announce the Release

After assets and install flow are confirmed:

- update release notes on GitHub
- share the install command
- optionally share the pinned version install form

Latest install:

```bash
curl -fsSL https://raw.githubusercontent.com/AruNi-01/local-router/main/scripts/install/install.sh | sh
```

Pinned install:

```bash
curl -fsSL https://raw.githubusercontent.com/AruNi-01/local-router/main/scripts/install/install.sh -o /tmp/localrouter-install.sh
LOCALROUTER_VERSION=v0.1.0 bash /tmp/localrouter-install.sh
```

## 10. If the Release Is Bad

Do not silently reuse the same version after users may have seen it.

Preferred fix:

1. diagnose the issue
2. land a patch on `main`
3. cut a new tag like `v0.1.1`

If the release is completely unusable and must be removed:

```bash
git push --delete origin v0.1.0
git tag -d v0.1.0
```

Also delete the GitHub Release manually.

Only do this if you are certain nobody should consume that version.

## Notes

- `scripts/install/install.sh` expects `latest` assets to keep the `.tar.gz` suffix.
- The install script verifies SHA-256 before extracting.
- The daemon serves the embedded dashboard from `http://127.0.0.1:9731/`.
- End-user install flow depends on GitHub Release assets, not on the source tree.
