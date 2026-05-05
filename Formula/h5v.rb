class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.5.3"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.5.3/h5v-aarch64-apple-darwin-v0.5.3.tar.gz"
      sha256 "352ff1e6dc69b4e1b730fd9d6dc48c58b91f527783a8bf48dbe56ea23278d010"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.5.3/h5v-x86_64-unknown-linux-gnu-v0.5.3.tar.gz"
    sha256 "8a095619a505309637619b2eb3fd16b65d18eab4c9b22d9e344691e4e3dc1344"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
