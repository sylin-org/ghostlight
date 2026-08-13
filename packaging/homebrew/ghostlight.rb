# Ghostlight Homebrew tap template. scripts/prepare-package-manager-metadata.ps1 replaces every
# placeholder below from one checked release candidate before this formula is published to the tap.
class Ghostlight < Formula
  desc "Visible browser automation in your signed-in Chromium profile"
  homepage "https://sylin.org/ghostlight/"
  version "__VERSION__"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "__SHA_AARCH64_APPLE_DARWIN__"
    else
      url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "__SHA_X86_64_APPLE_DARWIN__"
    end
  end

  on_linux do
    url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "__SHA_X86_64_UNKNOWN_LINUX_GNU__"
  end

  def install
    bin.install "ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector"
    pkgshare.install "LICENSE", "MIT.txt", "LicenseRef-Ghostlight-Commercial.txt", "LICENSING.md"
  end

  def caveats
    <<~EOS
      Connect Ghostlight to browsers and detected MCP clients:
        ghostlight install
      Then install Ghostlight in Browser. Run `ghostlight doctor` to verify the connection.
      License details are installed under #{pkgshare}.
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/ghostlight --version")
  end
end
