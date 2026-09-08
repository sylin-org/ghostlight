# Copy to an ignored file such as .tmp/linux-kde.env, edit, then source it before run.sh.
# Use a separate area for each machine/environment. No generated state belongs in tests/linux/.
export GHOSTLIGHT_LINUX_AREA="$PWD/.tmp/linux-kde"
export GHOSTLIGHT_LINUX_INSTALLED_AREA="$PWD/.tmp/linux-kde-installed"
export GHOSTLIGHT_LINUX_CHROMIUM=/usr/lib/chromium/chromium
# Native browser executable, never a Snap or Flatpak launcher.
export GHOSTLIGHT_LINUX_BRAVE="$GHOSTLIGHT_LINUX_AREA/brave/opt/brave.com/brave/brave"

# Optional: point to already-built/extracted artifacts instead of the default area layout.
# export GHOSTLIGHT_LINUX_BIN_DIR="$PWD/.tmp/candidate/bin"
# export GHOSTLIGHT_LINUX_PORTABLE_ARCHIVE="$PWD/.tmp/candidate/portable.tar.gz"
# export GHOSTLIGHT_LINUX_DEB="$PWD/.tmp/candidate/ghostlight.deb"
# export GHOSTLIGHT_LINUX_PREVIOUS_DEB="$PWD/.tmp/predecessor/ghostlight.deb"
export GHOSTLIGHT_LINUX_PREVIOUS_VERSION=1.3.4
# Candidate version defaults to Cargo.toml. This predecessor pin reproduces the recorded lane.

# Optional build-guest tool locations. The Rust toolchain defaults to rustup's active toolchain.
# export GHOSTLIGHT_LINUX_GUEST_AREA="$GHOSTLIGHT_LINUX_AREA/guest"
# export GHOSTLIGHT_LINUX_POWERSHELL_DIR=/path/to/powershell-7.6.5
# export GHOSTLIGHT_LINUX_RUST_TOOLCHAIN_DIR=/path/to/rust-toolchain
# export GHOSTLIGHT_LINUX_CARGO_HOME="$HOME/.cargo"
# export GHOSTLIGHT_LINUX_EXTENSION="$PWD/extension"
