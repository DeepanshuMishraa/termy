fn main() {
    println!("cargo::rustc-check-cfg=cfg(macos_sdk_26)");

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let output = Command::new("xcrun")
            .args(["--sdk", "macosx", "--show-sdk-version"])
            .output()
            .expect("failed to query macOS SDK version with xcrun");

        let sdk_version = String::from_utf8(output.stdout)
            .expect("failed to parse macOS SDK version output as UTF-8");

        let major_version: Option<u32> = sdk_version
            .trim()
            .split('.')
            .next()
            .and_then(|v| v.parse().ok());

        if let Some(major) = major_version
            && major >= 26
        {
            println!("cargo:rustc-cfg=macos_sdk_26");
        }
    }

    #[cfg(target_os = "windows")]
    {
        let icon_path = "assets/termy.ico";
        if std::path::Path::new(icon_path).exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon(icon_path);
            if let Err(err) = res.compile() {
                panic!("failed to compile Windows resources: {err}");
            }
        }
    }
}
