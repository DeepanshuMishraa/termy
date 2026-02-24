cask "termy" do
  arch arm: "arm64", intel: "x86_64"

  version "0.1.29"
  sha256 arm:   "ceda4d7bb220221453786ffab9dcf1d20b689b18183fa6b105d238e1d639001c",
         intel: "fe011a85c80c6b8ec821eb4de908e1fcec2844779ed7566f292bbcb48e3746fe"

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
