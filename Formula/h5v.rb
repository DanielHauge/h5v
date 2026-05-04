class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.4.4"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.4.4/h5v-aarch64-apple-darwin-v0.4.4.tar.gz"
      sha256 "d72f942fa7f87bdbaa4f9a84af2e6c9941978b3677a215a543c5a731e01bb2a7"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.4.4/h5v-x86_64-unknown-linux-gnu-v0.4.4.tar.gz"
    sha256 "61e572c9afe2d67125c59099c9bb51ad97348f431ed18c24a28cfd30994b95ae"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
