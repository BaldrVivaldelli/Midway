//! Tokens visuales compartidos por la interfaz de `midway-desktop`.
#![allow(dead_code)]

use iced::Color;
use midway_core::domain::http::HttpMethod;

// ─── HSL → RGB helper ──────────────────────────────────────────────────────

/// Converts HSL (h in degrees 0–360, s and l in 0.0–1.0) to an iced `Color`.
fn hsl_to_color(h: f32, s: f32, l: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let (r1, g1, b1) = if h_prime < 1.0 {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = l - c / 2.0;
    Color::from_rgb(r1 + m, g1 + m, b1 + m)
}

/// Extracts HSL hue (0–360) from an iced `Color` (assumed linear sRGB 0–1).
pub fn color_to_hsl(color: Color) -> (f32, f32, f32) {
    let r = color.r;
    let g = color.g;
    let b = color.b;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < 1e-6 {
        let mut h = (g - b) / d;
        if h < 0.0 {
            h += 6.0;
        }
        h * 60.0
    } else if (max - g).abs() < 1e-6 {
        ((b - r) / d + 2.0) * 60.0
    } else {
        ((r - g) / d + 4.0) * 60.0
    };
    (h, s, l)
}

/// Modo de tema activo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    Light,
    /// Tema oscuro, usado por defecto (estilo Insomnia).
    #[default]
    Dark,
}

impl ThemeMode {
    pub fn toggled(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub background_primary: Color,
    pub background_secondary: Color,
    pub surface_elevated: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub border: Color,
    pub accent: Color,
    pub status_success: Color,
    pub status_error: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypographyScale {
    pub title: TextStyle,
    pub subtitle: TextStyle,
    pub body: TextStyle,
    pub secondary: TextStyle,
    pub monospace: TextStyle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub weight: iced::font::Weight,
    pub monospace: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpacingScale {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadiusScale {
    pub control: f32,
    pub card: f32,
    pub panel: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesignSystem {
    pub palette: Palette,
    pub typography: TypographyScale,
    pub spacing: SpacingScale,
    pub radius: RadiusScale,
}

impl DesignSystem {
    pub fn for_mode(mode: ThemeMode) -> Self {
        Self {
            palette: palette_for_mode(mode),
            typography: shared_typography(),
            spacing: shared_spacing(),
            radius: shared_radius(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectableItemStyle {
    pub text_color: Color,
    pub background: Option<Color>,
    pub indicator_color: Option<Color>,
}

pub fn selectable_item_style(ds: &DesignSystem, selected: bool) -> SelectableItemStyle {
    if selected {
        SelectableItemStyle {
            text_color: ds.palette.accent,
            background: Some(ds.palette.background_secondary),
            indicator_color: Some(ds.palette.accent),
        }
    } else {
        SelectableItemStyle {
            text_color: ds.palette.text_primary,
            background: None,
            indicator_color: None,
        }
    }
}

/// Returns a text color (white or black) that achieves ≥4.5:1 contrast ratio
/// against the given background color (WCAG AA). Uses relative luminance to
/// determine whether white or black text provides sufficient contrast.
pub fn contrast_text_color(background: Color) -> Color {
    // Linearize sRGB gamma for luminance calculation
    fn linearize(v: f32) -> f32 {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    let luminance = 0.2126 * linearize(background.r)
        + 0.7152 * linearize(background.g)
        + 0.0722 * linearize(background.b);
    // Compare contrast ratios: white on bg vs black on bg
    // Contrast ratio = (L1 + 0.05) / (L2 + 0.05) where L1 > L2
    // White luminance = 1.0, Black luminance = 0.0
    let contrast_with_white = (1.0 + 0.05) / (luminance + 0.05);
    let contrast_with_black = (luminance + 0.05) / (0.0 + 0.05);
    if contrast_with_white >= contrast_with_black {
        Color::WHITE
    } else {
        Color::BLACK
    }
}

/// Returns the status color band: neutral for 100–199, green for 200–399, red for 400–599.
pub fn status_color(ds: &DesignSystem, status: u16) -> Color {
    match status {
        100..=199 => ds.palette.text_secondary, // neutral/gray band
        200..=399 => ds.palette.status_success,
        400..=599 => ds.palette.status_error,
        _ => ds.palette.text_primary,
    }
}

/// Returns a color associated with the HTTP method (Insomnia-style).
///
/// Target hue ranges (HSL):
/// - GET:     90°–150° (green)        → chosen 120°
/// - POST:    25°–45°  (orange/amber)  → chosen 35°
/// - PUT:     190°–220° (blue)         → chosen 205°
/// - PATCH:   45°–65°  (yellow)        → chosen 55°
/// - DELETE:  0°–15°   (red)           → chosen 5°
/// - HEAD:    280°–320° (magenta/pink) → chosen 300°
/// - OPTIONS: 180°–190° (cyan)         → chosen 185°
///
/// Minimum pairwise hue separation ≥ 15° (circular).
pub fn method_color(method: HttpMethod) -> Color {
    match method {
        HttpMethod::GET => hsl_to_color(120.0, 0.60, 0.55),
        HttpMethod::POST => hsl_to_color(35.0, 0.75, 0.55),
        HttpMethod::PUT => hsl_to_color(205.0, 0.65, 0.55),
        HttpMethod::PATCH => hsl_to_color(55.0, 0.65, 0.50),
        HttpMethod::DELETE => hsl_to_color(5.0, 0.70, 0.55),
        HttpMethod::HEAD => hsl_to_color(300.0, 0.55, 0.60),
        HttpMethod::OPTIONS => hsl_to_color(185.0, 0.55, 0.55),
    }
}

fn palette_for_mode(mode: ThemeMode) -> Palette {
    match mode {
        ThemeMode::Light => Palette {
            background_primary: Color::from_rgb8(0xFA, 0xFA, 0xFA), // luminosity ~96% (>85%)
            background_secondary: Color::from_rgb8(0xF0, 0xF0, 0xF0),
            surface_elevated: Color::WHITE,
            text_primary: Color::from_rgb8(0x1A, 0x1A, 0x2E),
            text_secondary: Color::from_rgb8(0x6B, 0x6B, 0x80),
            border: Color::from_rgb8(0xE0, 0xE0, 0xE0),
            accent: hsl_to_color(265.0, 0.70, 0.55), // purple hue 265° (250–280 range)
            status_success: Color::from_rgb8(0x28, 0xA7, 0x45),
            status_error: Color::from_rgb8(0xDC, 0x35, 0x45),
        },
        ThemeMode::Dark => Palette {
            background_primary: Color::from_rgb8(0x1A, 0x1A, 0x28), // luminosity ~10% (<12%)
            background_secondary: Color::from_rgb8(0x22, 0x22, 0x34),
            surface_elevated: Color::from_rgb8(0x2D, 0x2D, 0x40),
            text_primary: Color::from_rgb8(0xE8, 0xE8, 0xF0),
            text_secondary: Color::from_rgb8(0x8E, 0x8E, 0xA8),
            border: Color::from_rgb8(0x3A, 0x3A, 0x50),
            accent: hsl_to_color(265.0, 0.70, 0.55), // purple hue 265° (250–280 range)
            status_success: Color::from_rgb8(0x6B, 0xCF, 0x8E),
            status_error: Color::from_rgb8(0xEF, 0x6C, 0x6C),
        },
    }
}

fn shared_typography() -> TypographyScale {
    TypographyScale {
        title: TextStyle {
            size: 22.0,
            weight: iced::font::Weight::Bold,
            monospace: false,
        },
        subtitle: TextStyle {
            size: 15.0,
            weight: iced::font::Weight::Semibold,
            monospace: false,
        },
        body: TextStyle {
            size: 13.0,
            weight: iced::font::Weight::Normal,
            monospace: false,
        },
        secondary: TextStyle {
            size: 11.0,
            weight: iced::font::Weight::Normal,
            monospace: false,
        },
        monospace: TextStyle {
            size: 13.0,
            weight: iced::font::Weight::Normal,
            monospace: true,
        },
    }
}

fn shared_spacing() -> SpacingScale {
    SpacingScale {
        xs: 4.0,
        sm: 8.0,
        md: 12.0,
        lg: 16.0,
        xl: 24.0,
    }
}

fn shared_radius() -> RadiusScale {
    RadiusScale {
        control: 6.0,
        card: 8.0,
        panel: 12.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use midway_core::domain::http::HttpMethod;

    /// Perceived luminosity (relative luminance) of an sRGB color (0–1 range).
    /// Uses the standard ITU-R BT.709 formula.
    fn perceived_luminosity(c: Color) -> f32 {
        // Linearize sRGB gamma
        fn linearize(v: f32) -> f32 {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linearize(c.r) + 0.7152 * linearize(c.g) + 0.0722 * linearize(c.b)
    }

    /// Circular hue distance in degrees.
    fn hue_distance(h1: f32, h2: f32) -> f32 {
        let d = (h1 - h2).abs();
        d.min(360.0 - d)
    }

    #[test]
    fn dark_background_luminosity_below_12_percent() {
        let ds = DesignSystem::for_mode(ThemeMode::Dark);
        let lum = perceived_luminosity(ds.palette.background_primary);
        assert!(
            lum < 0.12,
            "Dark background_primary luminosity {lum:.4} should be < 0.12"
        );
    }

    #[test]
    fn light_background_luminosity_above_85_percent() {
        let ds = DesignSystem::for_mode(ThemeMode::Light);
        let lum = perceived_luminosity(ds.palette.background_primary);
        assert!(
            lum > 0.85,
            "Light background_primary luminosity {lum:.4} should be > 0.85"
        );
    }

    #[test]
    fn accent_hue_is_purple_in_both_modes() {
        for mode in [ThemeMode::Dark, ThemeMode::Light] {
            let ds = DesignSystem::for_mode(mode);
            let (h, _s, _l) = color_to_hsl(ds.palette.accent);
            assert!(
                (250.0..=280.0).contains(&h),
                "Accent hue {h:.1}° for {mode:?} should be in [250, 280]"
            );
        }
    }

    #[test]
    fn method_color_hue_ranges() {
        let cases: &[(HttpMethod, f32, f32)] = &[
            (HttpMethod::GET, 90.0, 150.0),
            (HttpMethod::POST, 25.0, 45.0),
            (HttpMethod::PUT, 190.0, 220.0),
            (HttpMethod::PATCH, 45.0, 65.0),
            (HttpMethod::DELETE, 0.0, 15.0),
            (HttpMethod::HEAD, 280.0, 320.0),
            (HttpMethod::OPTIONS, 180.0, 190.0),
        ];
        for &(method, min_h, max_h) in cases {
            let c = method_color(method);
            let (h, _s, _l) = color_to_hsl(c);
            assert!(
                h >= min_h && h <= max_h,
                "{method:?} hue {h:.1}° not in [{min_h}, {max_h}]"
            );
        }
    }

    #[test]
    fn method_color_pairwise_separation_at_least_15_degrees() {
        let methods = HttpMethod::ALL;
        for (i, &m1) in methods.iter().enumerate() {
            for &m2 in &methods[i + 1..] {
                let (h1, _, _) = color_to_hsl(method_color(m1));
                let (h2, _, _) = color_to_hsl(method_color(m2));
                let dist = hue_distance(h1, h2);
                assert!(
                    dist >= 15.0,
                    "Hue separation {m1:?}({h1:.1}°) vs {m2:?}({h2:.1}°) = {dist:.1}° < 15°"
                );
            }
        }
    }

    #[test]
    fn status_color_bands() {
        let ds = DesignSystem::for_mode(ThemeMode::Dark);

        // 100-199: neutral (text_secondary)
        for s in [100, 150, 199] {
            assert_eq!(
                status_color(&ds, s),
                ds.palette.text_secondary,
                "status {s} should be neutral"
            );
        }
        // 200-399: green (status_success)
        for s in [200, 301, 399] {
            assert_eq!(
                status_color(&ds, s),
                ds.palette.status_success,
                "status {s} should be success"
            );
        }
        // 400-599: red (status_error)
        for s in [400, 500, 599] {
            assert_eq!(
                status_color(&ds, s),
                ds.palette.status_error,
                "status {s} should be error"
            );
        }
    }
}
