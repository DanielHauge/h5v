class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.5.0"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.5.0/h5v-aarch64-apple-darwin-v0.5.0.tar.gz"
      sha256 "ea92dc4003446d8ec9f2fcd53531b3cdf61711319bbd540ae13e2a3647b21bfa"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.5.0/h5v-x86_64-unknown-linux-gnu-v0.5.0.tar.gz"
    sha256 "c12ba33cc4b35a7ec314fc394a02df4bdcae27e6e1f9704e9a12aa470d6f63a5"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
