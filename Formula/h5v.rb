class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.5.1"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.5.1/h5v-aarch64-apple-darwin-v0.5.1.tar.gz"
      sha256 "96cb156944e0bae97f0b7a86b721489528dfc3482b0efb79dc7dc3d3d6da0800"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.5.1/h5v-x86_64-unknown-linux-gnu-v0.5.1.tar.gz"
    sha256 "14d3da0f4220d4f8339ce075606549c741924987ffaae0ea692f3b1746e28144"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
