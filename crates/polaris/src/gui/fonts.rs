//! The fixed, embedded typeface set (DESIGN.md: no font configuration, ever).
//! Swapping the product's typeface = replace the assets + edit this file only.

use iced::Font;

pub const WRITING_REGULAR_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/Newsreader16pt-Regular.ttf");
pub const WRITING_ITALIC_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/Newsreader16pt-Italic.ttf");
pub const WRITING_SEMIBOLD_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/Newsreader16pt-SemiBold.ttf");
pub const MONO_REGULAR_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/iAWriterMonoS-Regular.ttf");

/// Writing AND preview (decision 2026-07-08: one voice — Literata retired;
/// the mode switch is carried by the rendering, not a face change).
/// Newsreader 16pt optical size, decided 2026-07-06.
pub const WRITING: Font = Font::with_name("Newsreader 16pt");
/// Chrome (status line), code, source-literal contexts.
pub const MONO: Font = Font::with_name("iA Writer Mono S");
