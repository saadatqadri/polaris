//! The two fixed themes — design tokens from DESIGN.md, nothing configurable.

use iced::theme::Palette;
use iced::{Color, Theme};

#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    /// Page. Warm paper / warm near-black.
    pub bg: Color,
    /// Body text.
    pub ink: Color,
    /// Chrome, markdown syntax marks.
    pub quiet: Color,
    /// Rules, blockquote bars.
    #[allow(dead_code)] // styled markdown arrives in M4
    pub whisper: Color,
    /// The only accent: cursor and selection.
    pub star: Color,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb8(r, g, b)
}

pub const LIGHT: Tokens = Tokens {
    bg: rgb(0xFB, 0xFA, 0xF7),
    ink: rgb(0x24, 0x22, 0x1D),
    quiet: rgb(0xA9, 0xA4, 0x98),
    whisper: rgb(0xDE, 0xDA, 0xD1),
    star: rgb(0x4E, 0x6E, 0x8E),
};

pub const DARK: Tokens = Tokens {
    bg: rgb(0x1A, 0x19, 0x16),
    ink: rgb(0xDA, 0xD6, 0xCC),
    quiet: rgb(0x63, 0x5F, 0x54),
    whisper: rgb(0x33, 0x31, 0x2B),
    star: rgb(0x8F, 0xAE, 0xCB),
};

pub fn tokens(dark: bool) -> Tokens {
    if dark {
        DARK
    } else {
        LIGHT
    }
}

pub fn theme(dark: bool) -> Theme {
    let t = tokens(dark);
    Theme::custom(
        if dark {
            "Polaris Dark"
        } else {
            "Polaris Light"
        },
        Palette {
            background: t.bg,
            text: t.ink,
            primary: t.star,
            success: t.star,
            warning: t.star,
            danger: rgb(0xB0, 0x54, 0x4A),
        },
    )
}
