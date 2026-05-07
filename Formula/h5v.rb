class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.7.1"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.7.1/h5v-aarch64-apple-darwin-v0.7.1.tar.gz"
      sha256 "6475e1984ce02ed2a12475cdaa300741ef34443146677d67ea3df1def3fade53"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.7.1/h5v-x86_64-unknown-linux-gnu-v0.7.1.tar.gz"
    sha256 "73c9e7be0d6e8d078b9a0cd1c9d76dade1005ff0d19282b7bdd4ead92cbb6a47"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
