class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.8.0"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.8.0/h5v-aarch64-apple-darwin-v0.8.0.tar.gz"
      sha256 "27b5fc922166edc4cdb96c4aa12cd74f2863bd6b45ab770cdf63484dfe4231d1"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.8.0/h5v-x86_64-unknown-linux-gnu-v0.8.0.tar.gz"
    sha256 "9ae53b5056b70c1a449e6421480b6f3f68fc8539c4723e5ee063f64bf794d0a3"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
