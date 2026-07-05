//! The fixed, embedded typeface set (DESIGN.md: no font configuration, ever).
//! Swapping the product's typeface = replace the assets + edit this file only.

use iced::Font;

pub const SANS_REGULAR_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/InstrumentSans-Regular.ttf");
pub const SANS_ITALIC_BYTES: &[u8] = include_bytes!("../../assets/fonts/InstrumentSans-Italic.ttf");
pub const SANS_SEMIBOLD_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/InstrumentSans-SemiBold.ttf");
pub const MONO_REGULAR_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/iAWriterMonoS-Regular.ttf");

/// Writing mode (decision 2026-07-05: Instrument Sans, replacing iA Writer
/// Quattro).
pub const WRITING: Font = Font::with_name("Instrument Sans");
/// Chrome (status line), code, source-literal contexts.
pub const MONO: Font = Font::with_name("iA Writer Mono S");
