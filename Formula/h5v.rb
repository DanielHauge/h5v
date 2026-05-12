class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.8.2"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.8.2/h5v-aarch64-apple-darwin-v0.8.2.tar.gz"
      sha256 "a0b40a74355b2a530869fbc018f065cbfa22b24c04239e689df6243cb1561c31"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.8.2/h5v-x86_64-unknown-linux-gnu-v0.8.2.tar.gz"
    sha256 "f3ba50f57feec2024b0c38ea80689beebc7fc797b3dcc31055946e483bfed22f"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
