pub mod themes;

use ratatui::style::Color;
pub use themes::THEMES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub is_dark: bool,
    pub bg: Color,
    pub panel: Color,
    pub surface: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub assistant: Color,
    pub user: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub code_bg: Color,
    pub code_fg: Color,
    pub quote_fg: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
}

#[allow(dead_code)]
pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

pub const fn hex(hex: u32) -> Color {
    let r = ((hex >> 16) & 0xFF) as u8;
    let g = ((hex >> 8) & 0xFF) as u8;
    let b = (hex & 0xFF) as u8;
    Color::Rgb(r, g, b)
}

impl Theme {
    pub fn default_theme() -> &'static Theme {
        &THEMES[0]
    }

    pub fn from_name(name: &str) -> &'static Theme {
        let clean = name
            .trim()
            .to_lowercase()
            .replace('_', "-")
            .replace(' ', "-");

        // Exact match on ID
        if let Some(t) = THEMES.iter().find(|t| t.id == clean) {
            return t;
        }

        // Match on alias or partial name
        match clean.as_str() {
            "catppuccin" | "mocha" => Self::from_name("catppuccin-mocha"),
            "macchiato" => Self::from_name("catppuccin-macchiato"),
            "frappe" => Self::from_name("catppuccin-frappe"),
            "latte" => Self::from_name("catppuccin-latte"),
            "tokyo" | "tokyonight" => Self::from_name("tokyo-night"),
            "storm" => Self::from_name("tokyo-night-storm"),
            "moon" => Self::from_name("tokyo-night-moon"),
            "gruvbox" => Self::from_name("gruvbox-dark"),
            "rosepine" | "rose-pine-dark" => Self::from_name("rose-pine"),
            "solarized" => Self::from_name("solarized-dark"),
            "monokai" => Self::from_name("monokai-pro"),
            "ayu" => Self::from_name("ayu-dark"),
            "github" => Self::from_name("github-dark"),
            "flexoki" => Self::from_name("flexoki-dark"),
            _ => {
                // Substring match
                if let Some(t) = THEMES
                    .iter()
                    .find(|t| t.id.contains(&clean) || t.name.to_lowercase().contains(&clean))
                {
                    t
                } else {
                    Self::default_theme()
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn find_theme(name: &str) -> Option<&'static Theme> {
        let clean = name
            .trim()
            .to_lowercase()
            .replace('_', "-")
            .replace(' ', "-");
        if clean.is_empty() {
            return None;
        }
        THEMES.iter().find(|t| {
            t.id == clean
                || t.name.to_lowercase() == clean
                || t.id.contains(&clean)
                || t.name.to_lowercase().contains(&clean)
        })
    }
}

#[allow(dead_code)]
pub fn normalize_theme(name: &str) -> Option<&'static str> {
    let clean = name
        .trim()
        .to_lowercase()
        .replace('_', "-")
        .replace(' ', "-");
    Theme::find_theme(&clean).map(|t| t.id)
}

pub fn search_themes(query: &str) -> Vec<&'static Theme> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return THEMES.iter().collect();
    }
    THEMES
        .iter()
        .filter(|t| {
            t.id.contains(&q)
                || t.name.to_lowercase().contains(&q)
                || t.category.to_lowercase().contains(&q)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_catalog_has_over_50_themes() {
        assert!(
            THEMES.len() >= 50,
            "Expected at least 50 themes, found {}",
            THEMES.len()
        );
    }

    #[test]
    fn all_theme_ids_are_unique_and_non_empty() {
        let mut ids = std::collections::HashSet::new();
        for t in THEMES {
            assert!(!t.id.is_empty(), "Theme id must not be empty");
            assert!(!t.name.is_empty(), "Theme name must not be empty");
            assert!(ids.insert(t.id), "Duplicate theme id found: {}", t.id);
        }
    }

    #[test]
    fn theme_search_and_lookup_works() {
        let catppuccin = Theme::from_name("catppuccin-mocha");
        assert_eq!(catppuccin.id, "catppuccin-mocha");

        let tokyo = Theme::from_name("tokyo night");
        assert_eq!(tokyo.id, "tokyo-night");

        let results = search_themes("dracula");
        assert!(!results.is_empty());
        assert!(results.iter().any(|t| t.id == "dracula"));

        let retro_results = search_themes("retro");
        assert!(!retro_results.is_empty());
    }
}
