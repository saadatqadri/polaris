//! The fixed, embedded typeface set (DESIGN.md: no font configuration, ever).
//! Swapping the product's typeface = replace the assets + edit this file only.

use iced::Font;

pub const QUATTRO_REGULAR_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/iAWriterQuattroS-Regular.ttf");
pub const QUATTRO_BOLD_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/iAWriterQuattroS-Bold.ttf");
pub const QUATTRO_ITALIC_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/iAWriterQuattroS-Italic.ttf");
pub const MONO_REGULAR_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/iAWriterMonoS-Regular.ttf");

/// Writing mode.
pub const QUATTRO: Font = Font::with_name("iA Writer Quattro S");
/// Chrome (status line), code, source-literal contexts.
pub const MONO: Font = Font::with_name("iA Writer Mono S");
