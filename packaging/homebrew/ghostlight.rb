# Homebrew formula TEMPLATE for the sylin-org/homebrew-tap repository (Formula/ghostlight.rb).
# Fill the four sha256 values from the release's .sha256 assets, then push to the tap.
# Users: brew install sylin-org/tap/ghostlight
class Ghostlight < Formula
  desc "Governed browser automation over your own authenticated Chromium session (MCP)"
  homepage "https://sylin.org/ghostlight/"
  version "0.6.0"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "76f2a28cb30b06975df493dcc3bd36f7c41efce6cb623331a2c627df9d1d75b5"
    else
      url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "c26a38aa42cb57ea9a36570e3e81534691b12fc6af20b41f324f6fde3f4e84b4"
    end
  end

  on_linux do
    url "https://github.com/sylin-org/ghostlight/releases/download/v#{version}/ghostlight-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "b5dd1524ea022ebd5f061017bc291f07d76c1ab81d1938819be9bdb8e4bf72f4"
  end

  def install
    # ADR-0046 as amended by ADR-0051: two executables ship in the archive
    # (ghostlight + the single role-selected ghostlight-relay pass-through).
    bin.install "ghostlight", "ghostlight-relay"
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
