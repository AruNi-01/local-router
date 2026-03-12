#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
CLI_MANIFEST="${ROOT_DIR}/apps/localrouter-cli/Cargo.toml"

TARGET_TRIPLE="${1:-}"
PLATFORM_ID="${2:-}"

if [[ -z "${TARGET_TRIPLE}" || -z "${PLATFORM_ID}" ]]; then
  echo "usage: $0 <target-triple> <platform-id>" >&2
  echo "example: $0 x86_64-unknown-linux-gnu linux-x64" >&2
  exit 1
fi

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "${CLI_MANIFEST}" | head -n1)"
if [[ -z "${VERSION}" ]]; then
  echo "failed to detect version from ${CLI_MANIFEST}" >&2
  exit 1
fi

RELEASE_NAME="localrouter-v${VERSION}-${PLATFORM_ID}"
RELEASE_ROOT="${DIST_DIR}/${RELEASE_NAME}"
ARCHIVE_PATH="${DIST_DIR}/${RELEASE_NAME}.tar.gz"
CHECKSUM_PATH="${ARCHIVE_PATH}.sha256"

rm -rf "${RELEASE_ROOT}" "${ARCHIVE_PATH}" "${CHECKSUM_PATH}"
mkdir -p "${RELEASE_ROOT}/bin"

pushd "${ROOT_DIR}/apps/dashboard" >/dev/null
npm ci
npm run build
popd >/dev/null

pushd "${ROOT_DIR}" >/dev/null
cargo build --release --target "${TARGET_TRIPLE}" -p localrouter-cli -p localrouterd
popd >/dev/null

cp "${ROOT_DIR}/target/${TARGET_TRIPLE}/release/localrouter" "${RELEASE_ROOT}/bin/localrouter"
cp "${ROOT_DIR}/target/${TARGET_TRIPLE}/release/localrouterd" "${RELEASE_ROOT}/bin/localrouterd"
cp "${ROOT_DIR}/README.md" "${RELEASE_ROOT}/README.md"
cp "${ROOT_DIR}/README.zh-CN.md" "${RELEASE_ROOT}/README.zh-CN.md"
cp "${ROOT_DIR}/scripts/install/install.sh" "${RELEASE_ROOT}/install.sh"
chmod +x "${RELEASE_ROOT}/bin/localrouter" "${RELEASE_ROOT}/bin/localrouterd" "${RELEASE_ROOT}/install.sh"

tar -C "${DIST_DIR}" -czf "${ARCHIVE_PATH}" "${RELEASE_NAME}"
shasum -a 256 "${ARCHIVE_PATH}" | awk '{print $1}' > "${CHECKSUM_PATH}"

echo "built ${ARCHIVE_PATH}"
