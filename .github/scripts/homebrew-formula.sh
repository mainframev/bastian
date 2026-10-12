#!/bin/sh
# Print the Homebrew formula for a release.
# Usage: homebrew-formula.sh <version> <SHA256SUMS>
set -eu

VERSION=$1
SUMS=$2
BASE="https://github.com/mainframev/bastian/releases/download/v$VERSION"

sha() {
    sum=$(awk -v f="bastian-v$VERSION-$1.tar.gz" '$2 == f { print $1 }' "$SUMS")
    [ -n "$sum" ] || { echo "no checksum for $1 in $SUMS" >&2; exit 1; }
    echo "$sum"
}

asset() {
    sum=$(sha "$1")
    printf '      url "%s/bastian-v%s-%s.tar.gz"\n      sha256 "%s"\n' "$BASE" "$VERSION" "$1" "$sum"
}

# Resolved up front: a failure inside the heredoc wouldn't stop the script.
MAC_ARM=$(asset aarch64-apple-darwin)
MAC_INTEL=$(asset x86_64-apple-darwin)
LINUX_ARM=$(asset aarch64-unknown-linux-gnu)
LINUX_INTEL=$(asset x86_64-unknown-linux-gnu)

cat <<RUBY
class Bastian < Formula
  desc "Terminal Markdown reader"
  homepage "https://github.com/mainframev/bastian"
  license "MIT"

  on_macos do
    on_arm do
$MAC_ARM
    end
    on_intel do
$MAC_INTEL
    end
  end

  on_linux do
    on_arm do
$LINUX_ARM
    end
    on_intel do
$LINUX_INTEL
    end
  end

  def install
    bin.install "bastian"
    generate_completions_from_executable(bin/"bastian", "completions")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/bastian --version")
  end
end
RUBY
