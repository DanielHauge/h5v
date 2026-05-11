class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.8.0"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.8.0/h5v-aarch64-apple-darwin-v0.8.0.tar.gz"
      sha256 "0cb5e1722768716f84394c7a397dc898cace365c80b6c05847cd3a01169fae3f"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.8.0/h5v-x86_64-unknown-linux-gnu-v0.8.0.tar.gz"
    sha256 "203d792b371a18524ca4ed02ea1551656e01a53f717c7584e0f02714db0990bb"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
