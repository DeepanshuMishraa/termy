const BUILTIN_THEMES: &[&str] = &[
    "termy",
    "tokyo-night",
    "catppuccin-mocha",
    "dracula",
    "gruvbox-dark",
    "nord",
    "solarized-dark",
    "one-dark",
    "monokai",
    "material-dark",
    "palenight",
    "tomorrow-night",
    "oceanic-next",
];

pub fn run() {
    for theme in BUILTIN_THEMES {
        println!("{}", theme);
    }
}
