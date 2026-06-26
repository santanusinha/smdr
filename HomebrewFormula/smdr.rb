class Smdr < Formula
  desc "Simple Markdown Reader — fast native markdown viewer with vim-style navigation, live reload, and 22 themes"
  homepage "https://github.com/santanusinha/smdr"
  url "https://github.com/santanusinha/smdr/archive/refs/tags/v0.1.2.tar.gz"
  sha256 "a57e7c772d1de204a10f90df932023d1dde639a2b641ca42e97e4531a0ee474b"
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
