# Homebrew formula TEMPLATE for the sylin-org/homebrew-tap repository (Formula/ghostlight.rb).
# Fill the four sha256 values from the release's .sha256 assets, then push to the tap.
# Users: brew install sylin-org/tap/ghostlight
class Ghostlight < Formula
  desc "Governed browser automation over your own authenticated Chromium session (MCP)"
  homepage "https://sylin.org/ghostlight/"
  version "0.7.3"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "0846df68a843b1dfc2c35261a8a574a937579caa059c8a8167c43d24c18ebfbf"
    else
      url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "40c975b81c894420480aa15aa5fa8cd6e695b7fe41ed48c7651516d02ceab5ec"
    end
  end

  on_linux do
    url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "6b6ff0d88f96f3b0a74c4e5f09a70e5f5c63ae0fb5968c93da229a33006aadc9"
  end

  def install
    # ADR-0096: service, protocol-versioned MCP edge, and browser-only native relay.
    bin.install "ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector"
  end

  def caveats
    <<~EOS
      Connect the browser side (idempotent):
        ghostlight install
      then add the "Ghostlight in Browser" extension.
      Walkthrough: https://sylin.org/ghostlight/
    EOS
  end

  test do
    system "#{bin}/ghostlight", "--version"
  end
end
