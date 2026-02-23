cask "termy" do
  arch arm: "arm64", intel: "x86_64"

  version "0.1.28"
  sha256 arm:   "e573efd74b355590745478fdd2d63a580070d53383f7b93b93ba4764a6ccc74f",
         intel: "9c7d05a0c81187af4d9c626b41d336cd6a5b0995710c98741f161226b5bc4402"

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
