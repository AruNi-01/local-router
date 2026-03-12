#!/usr/bin/env bash
set -euo pipefail

REPO="${LOCALROUTER_REPO:-AruNi-01/local-router}"
VERSION="${LOCALROUTER_VERSION:-latest}"
INSTALL_DIR="${LOCALROUTER_INSTALL_DIR:-$HOME/.local/bin}"
BASE_URL="${LOCALROUTER_BASE_URL:-}"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

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

download_url() {
  local platform version resolved
  platform="$1"
  version="$2"

  if [[ -n "${BASE_URL}" ]]; then
    if [[ "${version}" == "latest" ]]; then
      echo "${BASE_URL}/localrouter-latest-${platform}.tar.gz"
    else
      resolved="${version#v}"
      echo "${BASE_URL}/localrouter-v${resolved}-${platform}.tar.gz"
    fi
    return
  fi

  if [[ "${version}" == "latest" ]]; then
    echo "https://github.com/${REPO}/releases/latest/download/localrouter-latest-${platform}.tar.gz"
  else
    resolved="${version#v}"
    echo "https://github.com/${REPO}/releases/download/v${resolved}/localrouter-v${resolved}-${platform}.tar.gz"
  fi
}

checksum_url() {
  local archive_url="$1"
  echo "${archive_url}.sha256"
}

verify_checksum() {
  local archive_path="$1"
  local expected="$2"
  local actual

  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "${archive_path}" | awk '{print $1}')"
  elif command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "${archive_path}" | awk '{print $1}')"
  elif command -v openssl >/dev/null 2>&1; then
    actual="$(openssl dgst -sha256 "${archive_path}" | awk '{print $NF}')"
  else
    echo "warning: no sha256 tool found; skipping checksum verification" >&2
    return 0
  fi

  if [[ "${actual}" != "${expected}" ]]; then
    echo "checksum mismatch for ${archive_path}" >&2
    echo "expected: ${expected}" >&2
    echo "actual:   ${actual}" >&2
    exit 1
  fi
}

PLATFORM="$(detect_platform)"
ARCHIVE_URL="$(download_url "${PLATFORM}" "${VERSION}")"
ARCHIVE_PATH="${TMP_DIR}/localrouter.tar.gz"
CHECKSUM_URL="$(checksum_url "${ARCHIVE_URL}")"
CHECKSUM_PATH="${TMP_DIR}/localrouter.tar.gz.sha256"

mkdir -p "${INSTALL_DIR}"

echo "downloading ${ARCHIVE_URL}"
curl -fsSL "${ARCHIVE_URL}" -o "${ARCHIVE_PATH}"
curl -fsSL "${CHECKSUM_URL}" -o "${CHECKSUM_PATH}"
verify_checksum "${ARCHIVE_PATH}" "$(tr -d '\n\r ' < "${CHECKSUM_PATH}")"

tar -xzf "${ARCHIVE_PATH}" -C "${TMP_DIR}"
PACKAGE_DIR="$(find "${TMP_DIR}" -maxdepth 1 -type d -name 'localrouter-v*' | head -n1)"

if [[ -z "${PACKAGE_DIR}" ]]; then
  echo "failed to unpack localrouter archive" >&2
  exit 1
fi

install -m 0755 "${PACKAGE_DIR}/bin/localrouter" "${INSTALL_DIR}/localrouter"
install -m 0755 "${PACKAGE_DIR}/bin/localrouterd" "${INSTALL_DIR}/localrouterd"

echo "installed localrouter and localrouterd to ${INSTALL_DIR}"

case ":${PATH}:" in
  *":${INSTALL_DIR}:"*)
    echo "next step: localrouter dev"
    ;;
  *)
    echo "next step:"
    echo "  ${INSTALL_DIR}/localrouter dev"
    echo
    echo "or add it to PATH first:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    echo "  localrouter dev"
    ;;
esac
