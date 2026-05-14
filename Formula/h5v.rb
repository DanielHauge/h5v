class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.9.0"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.9.0/h5v-aarch64-apple-darwin-v0.9.0.tar.gz"
      sha256 "3489050e47bd57a35f31e466c3a4f516402db8a58bfeed502f72c5b435418638"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.9.0/h5v-x86_64-unknown-linux-gnu-v0.9.0.tar.gz"
    sha256 "7ef50fe1effd8851cc45c2f6dcda47ffc6ce23eef38b2c1f89d106ae43fc554c"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
