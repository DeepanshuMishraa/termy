#[cfg(target_os = "windows")]
fn main() {
    let icon_path = "assets/termy.ico";
    if std::path::Path::new(icon_path).exists() {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(icon_path);
        if let Err(err) = res.compile() {
            panic!("failed to compile Windows resources: {err}");
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {}
