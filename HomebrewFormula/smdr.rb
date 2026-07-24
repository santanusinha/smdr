class Smdr < Formula
  desc "Simple Markdown Reader — fast native markdown viewer with vim-style navigation, live reload, and 22 themes"
  homepage "https://github.com/santanusinha/smdr"
  url "https://github.com/santanusinha/smdr/archive/refs/tags/v0.1.6.tar.gz"
  sha256 "25f6459b5712170883f59317da76976082e8ff6b8aaa1a72baf41f84404d7dcf"
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
