#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FORMULA_SOURCE="${1:-${ROOT_DIR}/dist/homebrew/localrouter.rb}"
TAP_REPO="${HOMEBREW_TAP_REPO:-}"
TAP_TOKEN="${HOMEBREW_TAP_TOKEN:-}"
TAP_BRANCH="${HOMEBREW_TAP_BRANCH:-main}"
FORMULA_TARGET_PATH="${HOMEBREW_TAP_FORMULA_PATH:-Formula/localrouter.rb}"
README_SOURCE="${HOMEBREW_TAP_README_SOURCE:-${ROOT_DIR}/dist/homebrew/README.md}"
README_TARGET_PATH="${HOMEBREW_TAP_README_PATH:-README.md}"
UPDATE_README="${HOMEBREW_TAP_UPDATE_README:-true}"
GIT_NAME="${HOMEBREW_TAP_GIT_NAME:-localrouter-bot}"
GIT_EMAIL="${HOMEBREW_TAP_GIT_EMAIL:-localrouter-bot@users.noreply.github.com}"

if [[ -z "${TAP_REPO}" || -z "${TAP_TOKEN}" ]]; then
  echo "homebrew tap automation is not configured; skipping"
  exit 0
fi

if [[ ! -f "${FORMULA_SOURCE}" ]]; then
  echo "formula source not found: ${FORMULA_SOURCE}" >&2
  exit 1
fi

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

REMOTE_URL="https://x-access-token:${TAP_TOKEN}@github.com/${TAP_REPO}.git"
TARGET_DIR="${TMP_DIR}/tap"

git clone --depth 1 --branch "${TAP_BRANCH}" "${REMOTE_URL}" "${TARGET_DIR}" >/dev/null 2>&1
mkdir -p "$(dirname "${TARGET_DIR}/${FORMULA_TARGET_PATH}")"
cp "${FORMULA_SOURCE}" "${TARGET_DIR}/${FORMULA_TARGET_PATH}"
if [[ "${UPDATE_README}" == "true" ]]; then
  if [[ ! -f "${README_SOURCE}" ]]; then
    echo "tap README source not found: ${README_SOURCE}" >&2
    exit 1
  fi
  mkdir -p "$(dirname "${TARGET_DIR}/${README_TARGET_PATH}")"
  cp "${README_SOURCE}" "${TARGET_DIR}/${README_TARGET_PATH}"
fi

pushd "${TARGET_DIR}" >/dev/null
git config user.name "${GIT_NAME}"
git config user.email "${GIT_EMAIL}"

DIFF_PATHS=("${FORMULA_TARGET_PATH}")
if [[ "${UPDATE_README}" == "true" ]]; then
  DIFF_PATHS+=("${README_TARGET_PATH}")
fi

if git diff --quiet -- "${DIFF_PATHS[@]}"; then
  echo "homebrew tap is already up to date"
  popd >/dev/null
  exit 0
fi

git add "${FORMULA_TARGET_PATH}"
if [[ "${UPDATE_README}" == "true" ]]; then
  git add "${README_TARGET_PATH}"
fi
git commit -m "localrouter $(sed -n 's/^  version \"\\(.*\\)\"/\\1/p' "${FORMULA_TARGET_PATH}" | head -n1)" >/dev/null
git push origin "${TAP_BRANCH}" >/dev/null
popd >/dev/null

echo "updated homebrew tap ${TAP_REPO}:${FORMULA_TARGET_PATH}"
