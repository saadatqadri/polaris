import SwiftUI

struct WritingView: View {
    @Binding var document: MarkdownDocument
    @Environment(\.colorScheme) private var scheme

    @State private var preview = false
    @State private var hemingway = false
    @State private var typewriter = false

    private var dark: Bool { scheme == .dark }

    var body: some View {
        VStack(spacing: 0) {
            chrome
                .padding(.horizontal, 24)
                .padding(.top, 14)
                .padding(.bottom, 8)

            Group {
                if preview {
                    PreviewView(markdown: document.text, dark: dark)
                        .transition(.opacity)
                } else {
                    PolarisTextView(
                        text: $document.text, dark: dark,
                        hemingway: hemingway, typewriter: typewriter
                    )
                    .frame(maxWidth: 620)
                    .frame(maxWidth: .infinity)
                    .padding(.horizontal, 24)
                    .transition(.opacity)
                }
            }
        }
        .background(Tokens.bg(dark))
        // The summoned command surface + keyboard shortcuts (DESIGN.md).
        .overlay(alignment: .bottomTrailing) { modesControl }
        .background(shortcuts)
    }

    // A quiet floating control that summons the modes menu.
    private var modesControl: some View {
        Menu {
            toggle("Preview", "p", systemImage: "doc.richtext", on: preview) {
                setPreview(!preview)
            }
            Divider()
            toggle("Typewriter", "y", systemImage: "text.aligncenter", on: typewriter) {
                typewriter.toggle()
            }
            toggle("Hemingway", "e", systemImage: "arrow.forward", on: hemingway) {
                hemingway.toggle()
            }
            Divider()
            Label("Focus — soon", systemImage: "circle.dashed")
        } label: {
            Image(systemName: "slider.horizontal.3")
                .font(.system(size: 17, weight: .medium))
                .foregroundStyle(Tokens.quiet(dark))
                .frame(width: 46, height: 46)
                .background(Tokens.bg(dark))
                .clipShape(Circle())
                .overlay(Circle().stroke(Tokens.quiet(dark).opacity(0.3), lineWidth: 1))
        }
        .padding(24)
    }

    // Hidden buttons keep the shortcuts in the responder chain (and in the
    // hold-⌘ overlay) even when the menu is closed.
    private var shortcuts: some View {
        ZStack {
            Button("Preview") { setPreview(!preview) }
                .keyboardShortcut("p", modifiers: .command)
            Button("Typewriter") { typewriter.toggle() }
                .keyboardShortcut("y", modifiers: .command)
            Button("Hemingway") { hemingway.toggle() }
                .keyboardShortcut("e", modifiers: .command)
        }
        .opacity(0)
    }

    private func toggle(
        _ title: String, _ key: KeyEquivalent, systemImage: String, on: Bool,
        _ action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Label(on ? "\(title) ✓" : title, systemImage: systemImage)
        }
        .keyboardShortcut(key, modifiers: .command)
    }

    private func setPreview(_ on: Bool) {
        withAnimation(.easeInOut(duration: 0.15)) { preview = on }
    }

    private var chrome: some View {
        let words = Int(wordCount(text: document.text))
        let mins = max(1, Int(ceil(Double(words) / 220.0)))
        var parts = [words > 0 ? "\(words) words · \(mins) min" : "empty"]
        if preview { parts.append("preview") }
        if typewriter { parts.append("typewriter") }
        if hemingway { parts.append("hemingway") }
        return HStack {
            Text("polaris")
                .font(Fonts.mono(13))
                .foregroundStyle(Tokens.quiet(dark))
            Spacer()
            Text(parts.joined(separator: " · "))
                .font(Fonts.mono(13))
                .foregroundStyle(Tokens.quiet(dark))
        }
    }
}
