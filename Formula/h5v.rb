class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.13.0"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.13.0/h5v-aarch64-apple-darwin-v0.13.0.tar.gz"
      sha256 "59415abdd5a77a6ef40c4ec9740ec5f920a0cbfef37f2068ec93911d03300f35"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.13.0/h5v-x86_64-unknown-linux-gnu-v0.13.0.tar.gz"
    sha256 "5b4271b73d82897e2ac671f301c400f17e02ac72395a625cb163f03bf23fcb08"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
