use crate::config::{config_path, parse_theme_id};

// Theme color definitions (RGB values)
struct ThemeColors {
    foreground: (u8, u8, u8),
    background: (u8, u8, u8),
    cursor: (u8, u8, u8),
    ansi: [(u8, u8, u8); 16],
}

fn get_theme_colors(theme_id: &str) -> Option<ThemeColors> {
    match theme_id {
        "termy" => Some(ThemeColors {
            foreground: (231, 235, 245),
            background: (11, 16, 32),
            cursor: (167, 233, 163),
            ansi: [
                (11, 16, 32),      // black
                (241, 184, 197),   // red
                (167, 233, 163),   // green
                (255, 229, 163),   // yellow
                (163, 201, 233),   // blue
                (217, 163, 233),   // magenta
                (163, 233, 224),   // cyan
                (231, 235, 245),   // white
                (74, 85, 104),     // bright black
                (245, 198, 203),   // bright red
                (195, 233, 195),   // bright green
                (255, 243, 205),   // bright yellow
                (195, 217, 233),   // bright blue
                (233, 195, 245),   // bright magenta
                (195, 245, 240),   // bright cyan
                (255, 255, 255),   // bright white
            ],
        }),
        "tokyo-night" => Some(ThemeColors {
            foreground: (192, 202, 245),
            background: (26, 27, 38),
            cursor: (192, 202, 245),
            ansi: [
                (21, 22, 30), (247, 118, 142), (158, 206, 106), (224, 175, 104),
                (122, 162, 247), (187, 154, 247), (125, 207, 255), (192, 202, 245),
                (65, 72, 104), (247, 118, 142), (158, 206, 106), (224, 175, 104),
                (122, 162, 247), (187, 154, 247), (125, 207, 255), (192, 202, 245),
            ],
        }),
        "catppuccin-mocha" => Some(ThemeColors {
            foreground: (205, 214, 244),
            background: (30, 30, 46),
            cursor: (245, 224, 220),
            ansi: [
                (69, 71, 90), (243, 139, 168), (166, 227, 161), (249, 226, 175),
                (137, 180, 250), (203, 166, 247), (148, 226, 213), (186, 194, 222),
                (88, 91, 112), (243, 139, 168), (166, 227, 161), (249, 226, 175),
                (137, 180, 250), (203, 166, 247), (148, 226, 213), (166, 173, 200),
            ],
        }),
        "dracula" => Some(ThemeColors {
            foreground: (248, 248, 242),
            background: (40, 42, 54),
            cursor: (248, 248, 242),
            ansi: [
                (33, 34, 44), (255, 85, 85), (80, 250, 123), (241, 250, 140),
                (98, 114, 164), (255, 121, 198), (139, 233, 253), (248, 248, 242),
                (68, 71, 90), (255, 110, 110), (105, 255, 148), (255, 255, 165),
                (123, 139, 189), (255, 146, 223), (164, 255, 255), (255, 255, 255),
            ],
        }),
        "gruvbox-dark" => Some(ThemeColors {
            foreground: (235, 219, 178),
            background: (40, 40, 40),
            cursor: (235, 219, 178),
            ansi: [
                (40, 40, 40), (204, 36, 29), (152, 151, 26), (215, 153, 33),
                (69, 133, 136), (177, 98, 134), (104, 157, 106), (168, 153, 132),
                (146, 131, 116), (251, 73, 52), (184, 187, 38), (250, 189, 47),
                (131, 165, 152), (211, 134, 155), (142, 192, 124), (235, 219, 178),
            ],
        }),
        "nord" => Some(ThemeColors {
            foreground: (216, 222, 233),
            background: (46, 52, 64),
            cursor: (216, 222, 233),
            ansi: [
                (59, 66, 82), (191, 97, 106), (163, 190, 140), (235, 203, 139),
                (129, 161, 193), (180, 142, 173), (136, 192, 208), (229, 233, 240),
                (76, 86, 106), (191, 97, 106), (163, 190, 140), (235, 203, 139),
                (129, 161, 193), (180, 142, 173), (143, 188, 187), (236, 239, 244),
            ],
        }),
        "solarized-dark" => Some(ThemeColors {
            foreground: (131, 148, 150),
            background: (0, 43, 54),
            cursor: (131, 148, 150),
            ansi: [
                (7, 54, 66), (220, 50, 47), (133, 153, 0), (181, 137, 0),
                (38, 139, 210), (211, 54, 130), (42, 161, 152), (238, 232, 213),
                (0, 43, 54), (203, 75, 22), (88, 110, 117), (101, 123, 131),
                (131, 148, 150), (108, 113, 196), (147, 161, 161), (253, 246, 227),
            ],
        }),
        "one-dark" => Some(ThemeColors {
            foreground: (171, 178, 191),
            background: (40, 44, 52),
            cursor: (171, 178, 191),
            ansi: [
                (40, 44, 52), (224, 108, 117), (152, 195, 121), (229, 192, 123),
                (97, 175, 239), (198, 120, 221), (86, 182, 194), (171, 178, 191),
                (92, 99, 112), (224, 108, 117), (152, 195, 121), (229, 192, 123),
                (97, 175, 239), (198, 120, 221), (86, 182, 194), (255, 255, 255),
            ],
        }),
        "monokai" => Some(ThemeColors {
            foreground: (248, 248, 242),
            background: (39, 40, 34),
            cursor: (248, 248, 242),
            ansi: [
                (39, 40, 34), (249, 38, 114), (166, 226, 46), (244, 191, 117),
                (102, 217, 239), (174, 129, 255), (161, 239, 228), (248, 248, 242),
                (117, 113, 94), (249, 38, 114), (166, 226, 46), (244, 191, 117),
                (102, 217, 239), (174, 129, 255), (161, 239, 228), (249, 248, 245),
            ],
        }),
        "material-dark" => Some(ThemeColors {
            foreground: (238, 255, 255),
            background: (38, 50, 56),
            cursor: (238, 255, 255),
            ansi: [
                (84, 110, 122), (255, 83, 112), (195, 232, 141), (255, 203, 107),
                (130, 170, 255), (199, 146, 234), (137, 221, 255), (238, 255, 255),
                (84, 110, 122), (255, 83, 112), (195, 232, 141), (255, 203, 107),
                (130, 170, 255), (199, 146, 234), (137, 221, 255), (238, 255, 255),
            ],
        }),
        "palenight" => Some(ThemeColors {
            foreground: (166, 172, 205),
            background: (41, 45, 62),
            cursor: (255, 203, 107),
            ansi: [
                (41, 45, 62), (255, 85, 114), (195, 232, 141), (255, 203, 107),
                (130, 170, 255), (199, 146, 234), (137, 221, 255), (166, 172, 205),
                (103, 110, 149), (255, 85, 114), (195, 232, 141), (255, 203, 107),
                (130, 170, 255), (199, 146, 234), (137, 221, 255), (255, 255, 255),
            ],
        }),
        "tomorrow-night" => Some(ThemeColors {
            foreground: (197, 200, 198),
            background: (29, 31, 33),
            cursor: (197, 200, 198),
            ansi: [
                (29, 31, 33), (204, 102, 102), (181, 189, 104), (240, 198, 116),
                (129, 162, 190), (178, 148, 187), (138, 190, 183), (197, 200, 198),
                (150, 152, 150), (204, 102, 102), (181, 189, 104), (240, 198, 116),
                (129, 162, 190), (178, 148, 187), (138, 190, 183), (255, 255, 255),
            ],
        }),
        "oceanic-next" => Some(ThemeColors {
            foreground: (216, 222, 233),
            background: (27, 43, 52),
            cursor: (216, 222, 233),
            ansi: [
                (27, 43, 52), (236, 95, 103), (153, 199, 148), (250, 200, 99),
                (102, 153, 204), (197, 148, 197), (95, 179, 179), (216, 222, 233),
                (101, 115, 126), (236, 95, 103), (153, 199, 148), (250, 200, 99),
                (102, 153, 204), (197, 148, 197), (95, 179, 179), (255, 255, 255),
            ],
        }),
        _ => None,
    }
}

fn format_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

pub fn run() {
    // Get current theme from config
    let theme_id = if let Some(path) = config_path() {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            parse_theme_id(&contents).unwrap_or_else(|| "termy".to_string())
        } else {
            "termy".to_string()
        }
    } else {
        "termy".to_string()
    };

    let colors = match get_theme_colors(&theme_id) {
        Some(c) => c,
        None => {
            eprintln!("Unknown theme: {}. Using default.", theme_id);
            get_theme_colors("termy").unwrap()
        }
    };

    let (r, g, b) = colors.foreground;
    println!("foreground = {}", format_hex(r, g, b));

    let (r, g, b) = colors.background;
    println!("background = {}", format_hex(r, g, b));

    let (r, g, b) = colors.cursor;
    println!("cursor = {}", format_hex(r, g, b));

    let color_names = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
        "bright_black", "bright_red", "bright_green", "bright_yellow",
        "bright_blue", "bright_magenta", "bright_cyan", "bright_white",
    ];

    for (i, name) in color_names.iter().enumerate() {
        let (r, g, b) = colors.ansi[i];
        println!("{} = {}", name, format_hex(r, g, b));
    }
}
