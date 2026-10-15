class Smdr < Formula
  desc "Simple Markdown Reader — fast native markdown viewer with vim-style navigation, live reload, and 22 themes"
  homepage "https://github.com/santanusinha/smdr"
  url "https://github.com/santanusinha/smdr/archive/refs/tags/v0.1.7.tar.gz"
  sha256 "94239764873c6cf6538c90accf4a1cd2a567386c9a79ec40fcf7bfe7b32e5c90"
  license "MIT"
  head "https://github.com/santanusinha/smdr.git", branch: "master"


  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    # smdr opens a GUI window, so we just verify the binary exists and the
    # --help flag exits cleanly.
    system bin/"smdr", "--help"
  end
end
