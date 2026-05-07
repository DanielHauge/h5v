class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.7.0"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.7.0/h5v-aarch64-apple-darwin-v0.7.0.tar.gz"
      sha256 "6b0075a7d5c8b3417d50629933ce0b40d2a1f023f508f001a294e277f9484bbf"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.7.0/h5v-x86_64-unknown-linux-gnu-v0.7.0.tar.gz"
    sha256 "e33447835d726c58a9c2db1f1559506fe7d493e1c38c57e143d6133981952e62"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
