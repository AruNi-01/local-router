#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CLI_MANIFEST="${ROOT_DIR}/apps/localrouter-cli/Cargo.toml"
OUTPUT_PATH="${1:-${ROOT_DIR}/dist/homebrew/README.md}"
PROJECT_REPO="${LOCALROUTER_REPO:-AruNi-01/local-router}"
TAP_REPO="${HOMEBREW_TAP_REPO:-}"
FORMULA_NAME="${HOMEBREW_TAP_FORMULA_NAME:-localrouter}"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "${CLI_MANIFEST}" | head -n1)"

if [[ -z "${TAP_REPO}" ]]; then
  TAP_REPO="<owner>/homebrew-tap"
fi

mkdir -p "$(dirname "${OUTPUT_PATH}")"

cat > "${OUTPUT_PATH}" <<EOF
# Homebrew Tap for LocalRouter

This tap publishes the ${FORMULA_NAME} formula for LocalRouter.

## Install

    brew tap ${TAP_REPO}
    brew install ${FORMULA_NAME}

Or in a single command:

    brew install ${TAP_REPO}/${FORMULA_NAME}

## Upgrade

    brew update
    brew upgrade ${FORMULA_NAME}

## Verify

    ${FORMULA_NAME} daemon status

## Project

- Source repo: https://github.com/${PROJECT_REPO}
- Current formula version: ${VERSION}
EOF

echo "generated ${OUTPUT_PATH}"
