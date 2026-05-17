class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.11.0"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.11.0/h5v-aarch64-apple-darwin-v0.11.0.tar.gz"
      sha256 "4a4c625d231749070560f2ce0c2d053455e920785ae78718804d9af21ff367b4"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.11.0/h5v-x86_64-unknown-linux-gnu-v0.11.0.tar.gz"
    sha256 "7710b60817cc30ec6e36715b3d9ce4c51eee758e7600ad7f1b22f6938a5c4e35"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
