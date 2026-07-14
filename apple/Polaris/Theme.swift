// Polaris design tokens (design/DESIGN.md) for the iPad app — the same
// warm-paper / warm-near-black palette and fixed typefaces as desktop.

import SwiftUI

enum Tokens {
    static func bg(_ dark: Bool) -> Color {
        dark ? Color(red: 0x1A/255, green: 0x19/255, blue: 0x16/255)
             : Color(red: 0xFB/255, green: 0xFA/255, blue: 0xF7/255)
    }
    static func ink(_ dark: Bool) -> Color {
        dark ? Color(red: 0xDA/255, green: 0xD6/255, blue: 0xCC/255)
             : Color(red: 0x24/255, green: 0x22/255, blue: 0x1D/255)
    }
    static func quiet(_ dark: Bool) -> Color {
        dark ? Color(red: 0x63/255, green: 0x5F/255, blue: 0x54/255)
             : Color(red: 0xA9/255, green: 0xA4/255, blue: 0x98/255)
    }
    static func star(_ dark: Bool) -> Color {
        dark ? Color(red: 0x8F/255, green: 0xAE/255, blue: 0xCB/255)
             : Color(red: 0x4E/255, green: 0x6E/255, blue: 0x8E/255)
    }
}

enum Fonts {
    // Registered via UIAppFonts (Info.plist); referenced by PostScript name.
    // `.custom` silently falls back to the system face if the name is wrong.
    static func writing(_ size: CGFloat) -> Font { .custom("Newsreader16pt-Regular", size: size) }
    static func mono(_ size: CGFloat) -> Font { .custom("iAWriterMonoS-Regular", size: size) }
}
