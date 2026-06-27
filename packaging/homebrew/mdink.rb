# Homebrew formula for mdink — terminal markdown renderer.
#
# To install from a tap:
#   brew tap clicraft/mdink
#   brew install mdink
#
# SHA256 and version placeholders are updated per release by CI.

class Mdink < Formula
  desc "Terminal markdown renderer with syntax highlighting and image support"
  homepage "https://github.com/clicraft/mdink"
  version "VERSION_PLACEHOLDER"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/clicraft/mdink/releases/download/v#{version}/mdink-aarch64-apple-darwin.tar.gz"
      sha256 "SHA256_PLACEHOLDER_MACOS_ARM"
    end
    on_intel do
      url "https://github.com/clicraft/mdink/releases/download/v#{version}/mdink-x86_64-apple-darwin.tar.gz"
      sha256 "SHA256_PLACEHOLDER_MACOS_INTEL"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/clicraft/mdink/releases/download/v#{version}/mdink-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "SHA256_PLACEHOLDER_LINUX_ARM"
    end
    on_intel do
      url "https://github.com/clicraft/mdink/releases/download/v#{version}/mdink-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "SHA256_PLACEHOLDER_LINUX_INTEL"
    end
  end

  def install
    bin.install "mdink"
  end

  test do
    assert_match "mdink", shell_output("#{bin}/mdink --version")
  end
end
