# Homebrew formula TEMPLATE for the sylin-org/homebrew-tap repository (Formula/ghostlight.rb).
# Fill the four sha256 values from the release's .sha256 assets, then push to the tap.
# Users: brew install sylin-org/tap/ghostlight
class Ghostlight < Formula
  desc "Governed browser automation over your own authenticated Chromium session (MCP)"
  homepage "https://sylin-org.github.io/ghostlight/"
  version "0.5.0"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "TODO-SHA256-AARCH64-APPLE-DARWIN"
    else
      url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "TODO-SHA256-X86_64-APPLE-DARWIN"
    end
  end

  on_linux do
    url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "TODO-SHA256-X86_64-UNKNOWN-LINUX-GNU"
  end

  def install
    # ADR-0046: three role executables ship in the archive (ghostlight + the two adapters).
    bin.install "ghostlight", "ghostlight-adapter-agent", "ghostlight-adapter-browser"
  end

  def caveats
    <<~EOS
      Connect the browser side (idempotent):
        ghostlight install
      then add the "Ghostlight in Browser" extension.
      Walkthrough: https://sylin-org.github.io/ghostlight/install.html
    EOS
  end

  test do
    system "#{bin}/ghostlight", "--version"
  end
end
