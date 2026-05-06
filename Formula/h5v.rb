class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.6.0"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.6.0/h5v-aarch64-apple-darwin-v0.6.0.tar.gz"
      sha256 "5656522237eb8d05cc814372e59906d069f27bfc4949c24c7dde13cf4bcff53c"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.6.0/h5v-x86_64-unknown-linux-gnu-v0.6.0.tar.gz"
    sha256 "336c8f2984e1b864a61316aaa7cedb32ee536f0a63b846e61be35716dc28b78c"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
