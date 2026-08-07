# Homebrew formula TEMPLATE for the sylin-org/homebrew-tap repository (Formula/ghostlight.rb).
# Fill the four sha256 values from the release's .sha256 assets, then push to the tap.
# Users: brew install sylin-org/tap/ghostlight
class Ghostlight < Formula
  desc "Governed browser automation over your own authenticated Chromium session (MCP)"
  homepage "https://sylin.org/ghostlight/"
  version "0.8.0"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "ef19b7c69bc87845a892d8f05738c02b8cb54077c84407859078f1feb5ee41b2"
    else
      url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "3864ffcdc60123f6fb6ac2dee3886354ca5324b824ceda769dbd678107fcc6a5"
    end
  end

  on_linux do
    url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "cfccecd708a7a015b0a78723eb25b4ef15731161eb410c24bd40490b08d889f3"
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
