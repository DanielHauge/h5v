class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.15.0"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.15.0/h5v-aarch64-apple-darwin-v0.15.0.tar.gz"
      sha256 "0a6de30208fb4ab056ecbfeb353a91914632adca55357dd93ac7125c418d7b27"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.15.0/h5v-x86_64-unknown-linux-gnu-v0.15.0.tar.gz"
    sha256 "8fab485119906c965e7050fc4158c8e387e9d206652cb3d1ba7e67f6a845d0fd"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
