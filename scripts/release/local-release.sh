#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
STAGE_DIR="${LOCALROUTER_LOCAL_RELEASE_DIR:-${DIST_DIR}/local-release}"

detect_platform() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "${os}" in
    Darwin) os="darwin" ;;
    Linux) os="linux" ;;
    *)
      echo "unsupported OS: ${os}" >&2
      exit 1
      ;;
  esac

  case "${arch}" in
    x86_64|amd64) arch="x64" ;;
    arm64|aarch64) arch="arm64" ;;
    *)
      echo "unsupported architecture: ${arch}" >&2
      exit 1
      ;;
  esac

  echo "${os}-${arch}"
}

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "${os}:${arch}" in
    Darwin:arm64|Darwin:aarch64) echo "aarch64-apple-darwin" ;;
    Darwin:x86_64|Darwin:amd64) echo "x86_64-apple-darwin" ;;
    Linux:arm64|Linux:aarch64) echo "aarch64-unknown-linux-gnu" ;;
    Linux:x86_64|Linux:amd64) echo "x86_64-unknown-linux-gnu" ;;
    *)
      echo "unsupported platform ${os}/${arch}" >&2
      exit 1
      ;;
  esac
}

VERSION="$(
  sed -n 's/^version = "\(.*\)"/\1/p' "${ROOT_DIR}/apps/localrouter-cli/Cargo.toml" | head -n1
)"
PLATFORM_ID="${1:-$(detect_platform)}"
TARGET_TRIPLE="${2:-$(detect_target)}"
ARCHIVE_NAME="localrouter-v${VERSION}-${PLATFORM_ID}.tar.gz"
CHECKSUM_NAME="${ARCHIVE_NAME}.sha256"

"${ROOT_DIR}/scripts/release/build-release.sh" "${TARGET_TRIPLE}" "${PLATFORM_ID}"

rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}"
cp "${DIST_DIR}/${ARCHIVE_NAME}" "${STAGE_DIR}/${ARCHIVE_NAME}"
cp "${DIST_DIR}/${CHECKSUM_NAME}" "${STAGE_DIR}/${CHECKSUM_NAME}"
cp "${STAGE_DIR}/${ARCHIVE_NAME}" "${STAGE_DIR}/localrouter-latest-${PLATFORM_ID}.tar.gz"
cp "${STAGE_DIR}/${CHECKSUM_NAME}" "${STAGE_DIR}/localrouter-latest-${PLATFORM_ID}.tar.gz.sha256"

if ls "${DIST_DIR}"/localrouter-v"${VERSION}"-linux-x64.tar.gz \
      "${DIST_DIR}"/localrouter-v"${VERSION}"-linux-arm64.tar.gz \
      "${DIST_DIR}"/localrouter-v"${VERSION}"-darwin-x64.tar.gz \
      "${DIST_DIR}"/localrouter-v"${VERSION}"-darwin-arm64.tar.gz >/dev/null 2>&1; then
  RELEASE_FORMULA_DIR="${DIST_DIR}/release"
  rm -rf "${RELEASE_FORMULA_DIR}"
  mkdir -p "${RELEASE_FORMULA_DIR}"
  cp "${DIST_DIR}"/localrouter-v"${VERSION}"-*.tar.gz "${RELEASE_FORMULA_DIR}/"
  "${ROOT_DIR}/scripts/release/generate-homebrew-formula.sh" "${RELEASE_FORMULA_DIR}"
  cp "${DIST_DIR}/homebrew/localrouter.rb" "${STAGE_DIR}/localrouter.rb"
fi

cat <<EOF
local release staged in:
  ${STAGE_DIR}

local install test:
  LOCALROUTER_BASE_URL="file://${STAGE_DIR}" \\
  LOCALROUTER_INSTALL_DIR="\$(mktemp -d)" \\
  bash scripts/install/install.sh

installed binaries smoke test:
  LOCALROUTER_BASE_URL="file://${STAGE_DIR}" \\
  LOCALROUTER_INSTALL_DIR="\$(mktemp -d)" \\
  bash scripts/install/install.sh && "\$LOCALROUTER_INSTALL_DIR/localrouter" daemon status
EOF
