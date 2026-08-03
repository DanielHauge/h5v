class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.14.0"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.14.0/h5v-aarch64-apple-darwin-v0.14.0.tar.gz"
      sha256 "2a678840822bd0e808eb62448ee21cfc60bbfc19d12b13d23b933d2ae8627a8a"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.14.0/h5v-x86_64-unknown-linux-gnu-v0.14.0.tar.gz"
    sha256 "17ab3777bc319a4e320bad1451a411d9f005b4a245c6e5f134e12183050a29cb"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
