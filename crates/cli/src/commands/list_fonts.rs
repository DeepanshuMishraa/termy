#[cfg(target_os = "macos")]
pub fn run() {
    use core_text::font_collection::create_for_all_families;

    let collection = create_for_all_families();
    let descriptors = collection.get_descriptors();

    let mut fonts: Vec<String> = Vec::new();

    if let Some(descriptors) = descriptors {
        for i in 0..descriptors.len() {
            if let Some(descriptor) = descriptors.get(i) {
                let family_name = descriptor.family_name();
                if !fonts.contains(&family_name) {
                    fonts.push(family_name);
                }
            }
        }
    }

    fonts.sort();
    for font in fonts {
        println!("{}", font);
    }
}

#[cfg(target_os = "linux")]
pub fn run() {
    // Use fc-list command to get available fonts
    use std::process::Command;

    let output = Command::new("fc-list")
        .args([":spacing=mono", "-f", "%{family}\n"])
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut fonts: Vec<&str> = stdout.lines().collect();
                fonts.sort();
                fonts.dedup();
                for font in fonts {
                    if !font.is_empty() {
                        println!("{}", font);
                    }
                }
            } else {
                // Fallback to common monospace fonts
                print_common_monospace();
            }
        }
        Err(_) => {
            // fc-list not available, use fallback
            print_common_monospace();
        }
    }
}

#[cfg(target_os = "linux")]
fn print_common_monospace() {
    let common_monospace = [
        "DejaVu Sans Mono",
        "Liberation Mono",
        "Fira Code",
        "JetBrains Mono",
        "Source Code Pro",
        "Hack",
        "Inconsolata",
        "Ubuntu Mono",
        "Droid Sans Mono",
        "Roboto Mono",
        "Cascadia Code",
        "IBM Plex Mono",
    ];

    println!("Note: fc-list not available. Showing common monospace fonts:");
    for font in &common_monospace {
        println!("{}", font);
    }
}

#[cfg(target_os = "windows")]
pub fn run() {
    // On Windows, list common monospace fonts
    // Full DirectWrite enumeration requires more complex setup
    let common_monospace = [
        "Consolas",
        "Courier New",
        "Lucida Console",
        "Cascadia Code",
        "Cascadia Mono",
        "JetBrains Mono",
        "Fira Code",
        "Source Code Pro",
    ];

    for font in &common_monospace {
        println!("{}", font);
    }

    println!();
    println!("Note: This is a partial list of common monospace fonts.");
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn run() {
    eprintln!("Font listing is not supported on this platform");
}
