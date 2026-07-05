class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.13.1"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.13.1/h5v-aarch64-apple-darwin-v0.13.1.tar.gz"
      sha256 "40de9745c58038c3a86d90bf37f6f732001d96667963f3a7fefb58ed7b73fd77"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.13.1/h5v-x86_64-unknown-linux-gnu-v0.13.1.tar.gz"
    sha256 "a567da081db717e7d0cfffee47964b4f660273e9fd41390ce51af97929431d3e"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
