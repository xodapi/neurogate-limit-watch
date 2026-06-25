use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Btop,
    Dracula,
    Catppuccin,
    TokyoNight,
    Gruvbox,
    Nord,
    HighContrast,
    Protanopia,
    Deuteranopia,
    Tritanopia,
    Solarized,
    Monokai,
}

impl Theme {
    pub fn all() -> &'static [Theme] {
        &[
            Theme::Btop,
            Theme::Catppuccin,
            Theme::Deuteranopia,
            Theme::Dracula,
            Theme::Gruvbox,
            Theme::HighContrast,
            Theme::Monokai,
            Theme::Nord,
            Theme::Protanopia,
            Theme::Solarized,
            Theme::TokyoNight,
            Theme::Tritanopia,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Theme::Btop => "btop",
            Theme::Dracula => "dracula",
            Theme::Catppuccin => "catppuccin",
            Theme::TokyoNight => "tokyo-night",
            Theme::Gruvbox => "gruvbox",
            Theme::Nord => "nord",
            Theme::HighContrast => "high-contrast",
            Theme::Protanopia => "protanopia",
            Theme::Deuteranopia => "deuteranopia",
            Theme::Tritanopia => "tritanopia",
            Theme::Solarized => "solarized",
            Theme::Monokai => "monokai",
        }
    }

    pub fn from_name(name: &str) -> Option<Theme> {
        match name {
            "btop" => Some(Theme::Btop),
            "dracula" => Some(Theme::Dracula),
            "catppuccin" => Some(Theme::Catppuccin),
            "tokyo-night" | "tokyonight" | "tokyo_night" => Some(Theme::TokyoNight),
            "gruvbox" => Some(Theme::Gruvbox),
            "nord" => Some(Theme::Nord),
            "high-contrast" | "high_contrast" | "highcontrast" => Some(Theme::HighContrast),
            "protanopia" => Some(Theme::Protanopia),
            "deuteranopia" => Some(Theme::Deuteranopia),
            "tritanopia" => Some(Theme::Tritanopia),
            "solarized" => Some(Theme::Solarized),
            "monokai" => Some(Theme::Monokai),
            _ => None,
        }
    }

    pub fn palette(&self) -> Palette {
        match self {
            Theme::Btop => Palette {
                ok: Color::Green,
                warning: Color::Yellow,
                danger: Color::Red,
                header_bg: Color::DarkGray,
                border: Color::DarkGray,
                text: Color::White,
                muted: Color::DarkGray,
                gauge_ok: Color::Green,
                gauge_warning: Color::Yellow,
                gauge_danger: Color::Red,
                sparkline: Color::Cyan,
                key_bg: Color::Black,
                key_fg: Color::White,
                accent: Color::Cyan,
            },
            Theme::Dracula => Palette {
                ok: Color::Rgb(80, 250, 123),
                warning: Color::Rgb(241, 250, 128),
                danger: Color::Rgb(255, 85, 85),
                header_bg: Color::Rgb(68, 71, 90),
                border: Color::Rgb(98, 114, 164),
                text: Color::Rgb(248, 248, 242),
                muted: Color::Rgb(98, 114, 164),
                gauge_ok: Color::Rgb(80, 250, 123),
                gauge_warning: Color::Rgb(241, 250, 128),
                gauge_danger: Color::Rgb(255, 85, 85),
                sparkline: Color::Rgb(189, 147, 249),
                key_bg: Color::Rgb(68, 71, 90),
                key_fg: Color::Rgb(248, 248, 242),
                accent: Color::Rgb(139, 233, 253),
            },
            Theme::Catppuccin => Palette {
                ok: Color::Rgb(166, 227, 161),
                warning: Color::Rgb(249, 226, 175),
                danger: Color::Rgb(243, 139, 168),
                header_bg: Color::Rgb(49, 50, 68),
                border: Color::Rgb(88, 91, 112),
                text: Color::Rgb(205, 214, 244),
                muted: Color::Rgb(88, 91, 112),
                gauge_ok: Color::Rgb(166, 227, 161),
                gauge_warning: Color::Rgb(249, 226, 175),
                gauge_danger: Color::Rgb(243, 139, 168),
                sparkline: Color::Rgb(137, 180, 250),
                key_bg: Color::Rgb(49, 50, 68),
                key_fg: Color::Rgb(205, 214, 244),
                accent: Color::Rgb(148, 226, 213),
            },
            Theme::TokyoNight => Palette {
                ok: Color::Rgb(158, 206, 106),
                warning: Color::Rgb(224, 175, 104),
                danger: Color::Rgb(247, 118, 142),
                header_bg: Color::Rgb(30, 33, 48),
                border: Color::Rgb(59, 66, 97),
                text: Color::Rgb(192, 202, 245),
                muted: Color::Rgb(59, 66, 97),
                gauge_ok: Color::Rgb(158, 206, 106),
                gauge_warning: Color::Rgb(224, 175, 104),
                gauge_danger: Color::Rgb(247, 118, 142),
                sparkline: Color::Rgb(122, 162, 247),
                key_bg: Color::Rgb(30, 33, 48),
                key_fg: Color::Rgb(192, 202, 245),
                accent: Color::Rgb(125, 207, 255),
            },
            Theme::Gruvbox => Palette {
                ok: Color::Rgb(184, 187, 38),
                warning: Color::Rgb(250, 189, 47),
                danger: Color::Rgb(251, 73, 52),
                header_bg: Color::Rgb(40, 40, 40),
                border: Color::Rgb(124, 111, 100),
                text: Color::Rgb(235, 219, 178),
                muted: Color::Rgb(124, 111, 100),
                gauge_ok: Color::Rgb(184, 187, 38),
                gauge_warning: Color::Rgb(250, 189, 47),
                gauge_danger: Color::Rgb(251, 73, 52),
                sparkline: Color::Rgb(131, 165, 152),
                key_bg: Color::Rgb(40, 40, 40),
                key_fg: Color::Rgb(235, 219, 178),
                accent: Color::Rgb(183, 179, 106),
            },
            Theme::Nord => Palette {
                ok: Color::Rgb(163, 190, 140),
                warning: Color::Rgb(235, 203, 139),
                danger: Color::Rgb(191, 97, 106),
                header_bg: Color::Rgb(59, 66, 82),
                border: Color::Rgb(76, 86, 106),
                text: Color::Rgb(216, 222, 233),
                muted: Color::Rgb(76, 86, 106),
                gauge_ok: Color::Rgb(163, 190, 140),
                gauge_warning: Color::Rgb(235, 203, 139),
                gauge_danger: Color::Rgb(191, 97, 106),
                sparkline: Color::Rgb(129, 161, 193),
                key_bg: Color::Rgb(59, 66, 82),
                key_fg: Color::Rgb(216, 222, 233),
                accent: Color::Rgb(143, 188, 187),
            },
            Theme::HighContrast => Palette {
                ok: Color::Green,
                warning: Color::Yellow,
                danger: Color::Red,
                header_bg: Color::Black,
                border: Color::White,
                text: Color::White,
                muted: Color::Gray,
                gauge_ok: Color::Green,
                gauge_warning: Color::Yellow,
                gauge_danger: Color::Red,
                sparkline: Color::Cyan,
                key_bg: Color::White,
                key_fg: Color::Black,
                accent: Color::Cyan,
            },
            Theme::Protanopia => Palette {
                ok: Color::Rgb(100, 180, 246),
                warning: Color::Rgb(238, 210, 2),
                danger: Color::Rgb(255, 167, 38),
                header_bg: Color::Rgb(40, 42, 54),
                border: Color::Rgb(68, 71, 90),
                text: Color::Rgb(248, 248, 242),
                muted: Color::Rgb(68, 71, 90),
                gauge_ok: Color::Rgb(100, 180, 246),
                gauge_warning: Color::Rgb(238, 210, 2),
                gauge_danger: Color::Rgb(255, 167, 38),
                sparkline: Color::Rgb(189, 147, 249),
                key_bg: Color::Rgb(40, 42, 54),
                key_fg: Color::Rgb(248, 248, 242),
                accent: Color::Rgb(139, 233, 253),
            },
            Theme::Deuteranopia => Palette {
                ok: Color::Rgb(100, 180, 246),
                warning: Color::Rgb(255, 183, 77),
                danger: Color::Rgb(239, 83, 80),
                header_bg: Color::Rgb(40, 42, 54),
                border: Color::Rgb(68, 71, 90),
                text: Color::Rgb(248, 248, 242),
                muted: Color::Rgb(68, 71, 90),
                gauge_ok: Color::Rgb(100, 180, 246),
                gauge_warning: Color::Rgb(255, 183, 77),
                gauge_danger: Color::Rgb(239, 83, 80),
                sparkline: Color::Rgb(189, 147, 249),
                key_bg: Color::Rgb(40, 42, 54),
                key_fg: Color::Rgb(248, 248, 242),
                accent: Color::Rgb(139, 233, 253),
            },
            Theme::Tritanopia => Palette {
                ok: Color::Rgb(100, 221, 173),
                warning: Color::Rgb(239, 83, 80),
                danger: Color::Rgb(229, 57, 53),
                header_bg: Color::Rgb(40, 42, 54),
                border: Color::Rgb(68, 71, 90),
                text: Color::Rgb(248, 248, 242),
                muted: Color::Rgb(68, 71, 90),
                gauge_ok: Color::Rgb(100, 221, 173),
                gauge_warning: Color::Rgb(239, 83, 80),
                gauge_danger: Color::Rgb(229, 57, 53),
                sparkline: Color::Rgb(149, 117, 205),
                key_bg: Color::Rgb(40, 42, 54),
                key_fg: Color::Rgb(248, 248, 242),
                accent: Color::Rgb(139, 233, 253),
            },
            Theme::Solarized => Palette {
                ok: Color::Rgb(133, 153, 0),
                warning: Color::Rgb(181, 137, 0),
                danger: Color::Rgb(203, 75, 22),
                header_bg: Color::Rgb(0, 43, 54),
                border: Color::Rgb(7, 54, 66),
                text: Color::Rgb(253, 246, 227),
                muted: Color::Rgb(7, 54, 66),
                gauge_ok: Color::Rgb(133, 153, 0),
                gauge_warning: Color::Rgb(181, 137, 0),
                gauge_danger: Color::Rgb(203, 75, 22),
                sparkline: Color::Rgb(38, 139, 210),
                key_bg: Color::Rgb(0, 43, 54),
                key_fg: Color::Rgb(253, 246, 227),
                accent: Color::Rgb(42, 161, 152),
            },
            Theme::Monokai => Palette {
                ok: Color::Rgb(166, 226, 46),
                warning: Color::Rgb(230, 219, 100),
                danger: Color::Rgb(249, 38, 114),
                header_bg: Color::Rgb(39, 40, 34),
                border: Color::Rgb(85, 85, 85),
                text: Color::Rgb(248, 248, 242),
                muted: Color::Rgb(85, 85, 85),
                gauge_ok: Color::Rgb(166, 226, 46),
                gauge_warning: Color::Rgb(230, 219, 100),
                gauge_danger: Color::Rgb(249, 38, 114),
                sparkline: Color::Rgb(174, 129, 255),
                key_bg: Color::Rgb(39, 40, 34),
                key_fg: Color::Rgb(248, 248, 242),
                accent: Color::Rgb(102, 217, 239),
            },
        }
    }
}

#[allow(dead_code)]
pub struct Palette {
    pub ok: Color,
    pub warning: Color,
    pub danger: Color,
    pub header_bg: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub gauge_ok: Color,
    pub gauge_warning: Color,
    pub gauge_danger: Color,
    pub sparkline: Color,
    pub key_bg: Color,
    pub key_fg: Color,
    pub accent: Color,
}

#[allow(dead_code)]
impl Palette {
    pub fn level_color(&self, level: &str) -> Color {
        match level {
            "danger" => self.danger,
            "warning" => self.warning,
            _ => self.ok,
        }
    }

    pub fn level_style(&self, level: &str) -> Style {
        Style::default().fg(self.level_color(level))
    }

    pub fn bold_level_style(&self, level: &str) -> Style {
        self.level_style(level).add_modifier(Modifier::BOLD)
    }

    pub fn header_style(&self) -> Style {
        Style::default().fg(self.text).bg(self.header_bg)
    }

    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }

    pub fn gauge_style(&self, level: &str) -> Style {
        Style::default().fg(self.level_color(level))
    }

    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn accent_style(&self) -> Style {
        Style::default().fg(self.accent)
    }

    pub fn sparkline_style(&self) -> Style {
        Style::default().fg(self.sparkline)
    }

    pub fn key_binding_style(&self) -> Style {
        Style::default()
            .fg(self.key_fg)
            .bg(self.key_bg)
            .add_modifier(Modifier::BOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_themes_have_names() {
        for theme in Theme::all() {
            assert!(!theme.name().is_empty());
        }
    }

    #[test]
    fn from_name_roundtrips() {
        for theme in Theme::all() {
            let roundtripped = Theme::from_name(theme.name());
            assert_eq!(roundtripped, Some(*theme));
        }
    }

    #[test]
    fn all_palettes_produce_valid_colors() {
        for theme in Theme::all() {
            let palette = theme.palette();
            let style = palette.level_style("ok");
            assert!(style.fg.is_some());
            let style = palette.level_style("warning");
            assert!(style.fg.is_some());
            let style = palette.level_style("danger");
            assert!(style.fg.is_some());
        }
    }

    #[test]
    fn level_color_matches_level() {
        let palette = Theme::Btop.palette();
        assert_eq!(palette.level_color("ok"), Color::Green);
        assert_eq!(palette.level_color("warning"), Color::Yellow);
        assert_eq!(palette.level_color("danger"), Color::Red);
    }

    #[test]
    fn name_list_is_alphabetical() {
        let names: Vec<&str> = Theme::all().iter().map(|t| t.name()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn from_name_rejects_unknown() {
        assert_eq!(Theme::from_name("nonexistent"), None);
    }
}
