class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.13.2"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.13.2/h5v-aarch64-apple-darwin-v0.13.2.tar.gz"
      sha256 "505b4bf42a833ab69dfe535d3ec211986db0b76ec21beaf799166cdb9e9187c1"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.13.2/h5v-x86_64-unknown-linux-gnu-v0.13.2.tar.gz"
    sha256 "14556ee26ce0e601e4656e9afa7c9e1ca12d5d3a0cf8594039c94f0eed384248"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
