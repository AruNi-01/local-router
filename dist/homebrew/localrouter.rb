class Localrouter < Formula
  desc "Local development control plane with daemon, proxy, dashboard, and CLI"
  homepage "https://github.com/AruNi-01/local-router"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/AruNi-01/local-router/releases/download/v0.1.0/localrouter-v0.1.0-darwin-arm64.tar.gz"
      sha256 "749019b5d6f8ab79f7ff37a1fe4722dc9dd0484df7fa39d792018cb4f123593b"
    end
    on_intel do
      url "https://github.com/AruNi-01/local-router/releases/download/v0.1.0/localrouter-v0.1.0-darwin-x64.tar.gz"
      sha256 "749019b5d6f8ab79f7ff37a1fe4722dc9dd0484df7fa39d792018cb4f123593b"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/AruNi-01/local-router/releases/download/v0.1.0/localrouter-v0.1.0-linux-arm64.tar.gz"
      sha256 "749019b5d6f8ab79f7ff37a1fe4722dc9dd0484df7fa39d792018cb4f123593b"
    end
    on_intel do
      url "https://github.com/AruNi-01/local-router/releases/download/v0.1.0/localrouter-v0.1.0-linux-x64.tar.gz"
      sha256 "749019b5d6f8ab79f7ff37a1fe4722dc9dd0484df7fa39d792018cb4f123593b"
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
