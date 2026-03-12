#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CLI_MANIFEST="${ROOT_DIR}/apps/localrouter-cli/Cargo.toml"
RELEASE_DIR="${1:-${ROOT_DIR}/dist/release}"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "${CLI_MANIFEST}" | head -n1)"
REPO="${LOCALROUTER_REPO:-AruNi-01/local-router}"
FORMULA_DIR="${ROOT_DIR}/dist/homebrew"
FORMULA_PATH="${FORMULA_DIR}/localrouter.rb"

if [[ ! -d "${RELEASE_DIR}" ]]; then
  echo "release directory not found: ${RELEASE_DIR}" >&2
  exit 1
fi

asset_sha() {
  local name="$1"
  local path="${RELEASE_DIR}/${name}"
  if [[ ! -f "${path}" ]]; then
    echo "missing release asset: ${path}" >&2
    exit 1
  fi
  shasum -a 256 "${path}" | awk '{print $1}'
}

LINUX_X64_SHA="$(asset_sha "localrouter-v${VERSION}-linux-x64.tar.gz")"
LINUX_ARM64_SHA="$(asset_sha "localrouter-v${VERSION}-linux-arm64.tar.gz")"
DARWIN_X64_SHA="$(asset_sha "localrouter-v${VERSION}-darwin-x64.tar.gz")"
DARWIN_ARM64_SHA="$(asset_sha "localrouter-v${VERSION}-darwin-arm64.tar.gz")"

mkdir -p "${FORMULA_DIR}"

cat > "${FORMULA_PATH}" <<EOF
class Localrouter < Formula
  desc "Local development control plane with daemon, proxy, dashboard, and CLI"
  homepage "https://github.com/${REPO}"
  version "${VERSION}"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/${REPO}/releases/download/v${VERSION}/localrouter-v${VERSION}-darwin-arm64.tar.gz"
      sha256 "${DARWIN_ARM64_SHA}"
    end
    on_intel do
      url "https://github.com/${REPO}/releases/download/v${VERSION}/localrouter-v${VERSION}-darwin-x64.tar.gz"
      sha256 "${DARWIN_X64_SHA}"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/${REPO}/releases/download/v${VERSION}/localrouter-v${VERSION}-linux-arm64.tar.gz"
      sha256 "${LINUX_ARM64_SHA}"
    end
    on_intel do
      url "https://github.com/${REPO}/releases/download/v${VERSION}/localrouter-v${VERSION}-linux-x64.tar.gz"
      sha256 "${LINUX_X64_SHA}"
    end
  end

  def install
    bin.install "bin/localrouter"
    bin.install "bin/localrouterd"
    prefix.install "README.md"
    prefix.install "README.zh-CN.md"
  end

  test do
    assert_match "not running", shell_output("#{bin}/localrouter daemon status")
  end
end
EOF

echo "generated ${FORMULA_PATH}"
