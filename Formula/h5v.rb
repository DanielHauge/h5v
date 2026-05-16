class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.10.0"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.10.0/h5v-aarch64-apple-darwin-v0.10.0.tar.gz"
      sha256 "1869018b0e0800977cb88fe2f212b7c58ff9d797d9617461400b28611a53b9df"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.10.0/h5v-x86_64-unknown-linux-gnu-v0.10.0.tar.gz"
    sha256 "28a1e23738bfd5eb7a3f11cec6c32174c675c1a8d6fd00c300cf37ccd65c830f"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
