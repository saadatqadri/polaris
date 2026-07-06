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
pub const READING_REGULAR_BYTES: &[u8] = include_bytes!("../../assets/fonts/Literata-Regular.ttf");
pub const READING_ITALIC_BYTES: &[u8] = include_bytes!("../../assets/fonts/Literata-Italic.ttf");
pub const READING_SEMIBOLD_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/Literata-SemiBold.ttf");

/// Writing mode (decision 2026-07-06: Newsreader, 16pt optical size —
/// revising the Instrument Sans pick after daylight use).
pub const WRITING: Font = Font::with_name("Newsreader 16pt");
/// Chrome (status line), code, source-literal contexts.
pub const MONO: Font = Font::with_name("iA Writer Mono S");
/// Preview / reading mode.
pub const READING: Font = Font::with_name("Literata");
