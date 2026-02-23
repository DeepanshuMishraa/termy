cask "termy" do
  arch arm: "arm64", intel: "x86_64"

  version "0.1.27"
  sha256 arm:   "8a22b3c0c03fac71c0785a19527a951bc3ebb832a4f4ef04d16c8a306daff823",
         intel: "68de1545794d45a3db314942fd0f14faf3b886f6db6040cecfe27cc2f17c3a0a"

  url "https://github.com/lassejlv/termy/releases/download/v#{version}/Termy-v#{version}-macos-#{arch}.dmg"
  name "Termy"
  desc "Minimal GPU-powered terminal written in Rust"
  homepage "https://github.com/lassejlv/termy"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :big_sur"

  app "Termy.app"
end
