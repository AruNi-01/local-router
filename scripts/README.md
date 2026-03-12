# Scripts

Repository scripts are grouped by responsibility instead of living flat under `scripts/`.

## Layout

- `install/`
  - `install.sh`: install `localrouter` and `localrouterd` from a GitHub release or local staged release.
- `release/`
  - `build-release.sh`: build a release archive for a single target.
  - `local-release.sh`: stage a local release and print install smoke-test commands.
  - `generate-homebrew-formula.sh`: generate `localrouter.rb` from built release artifacts.
  - `generate-homebrew-tap-readme.sh`: generate the Homebrew tap README.
  - `update-homebrew-tap.sh`: push formula and README updates to the tap repository.

## Conventions

- Keep scripts in the narrowest directory that matches their purpose.
- Prefer calling scripts via `bash scripts/...` from the repository root in docs and CI.
- Release automation should live under `scripts/release/`.
- User-facing local or remote install entrypoints should live under `scripts/install/`.
