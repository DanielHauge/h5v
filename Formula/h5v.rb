class H5v < Formula
  desc "Terminal HDF5 viewer with matrix/chart/image previews"
  homepage "https://github.com/DanielHauge/h5v"
  version "0.5.2"
  license "Apache-2.0"

  on_macos do
      url "https://github.com/DanielHauge/h5v/releases/download/v0.5.2/h5v-aarch64-apple-darwin-v0.5.2.tar.gz"
      sha256 "37b43919d91660dba707bdc9b2069fea2cbfbc18f90bd7cc5e22721935a1962d"
  end

  on_linux do
    url "https://github.com/DanielHauge/h5v/releases/download/v0.5.2/h5v-x86_64-unknown-linux-gnu-v0.5.2.tar.gz"
    sha256 "f359450b5b91ba055398bbb0ae6344948c6f713e0046fc06864ff0bc8c77d4af"
  end

  def install
    bin.install "h5v"
  end

  test do
    assert_match "HDF5 terminal viewer", shell_output("#{bin}/h5v --help")
  end
end
