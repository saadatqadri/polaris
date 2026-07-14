import SwiftUI

struct WritingView: View {
    @Binding var document: MarkdownDocument
    @Environment(\.colorScheme) private var scheme
    @State private var preview = false

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
                    PolarisTextView(text: $document.text, dark: dark)
                        .frame(maxWidth: 620)
                        .frame(maxWidth: .infinity)
                        .padding(.horizontal, 24)
                        .transition(.opacity)
                }
            }
        }
        .background(Tokens.bg(dark))
        // Touch: horizontal swipe toggles write <-> preview.
        .gesture(
            DragGesture(minimumDistance: 40)
                .onEnded { g in
                    if abs(g.translation.width) > abs(g.translation.height) * 2 {
                        if g.translation.width < 0 { setPreview(true) } // swipe left -> read
                        else { setPreview(false) } // swipe right -> write
                    }
                }
        )
        // Keyboard: the same shortcut as desktop, documented in the hold-⌘
        // overlay. A zero-size button keeps it in the responder chain.
        .background(
            Button("Toggle Preview") { setPreview(!preview) }
                .keyboardShortcut("p", modifiers: .command)
                .opacity(0)
        )
    }

    private func setPreview(_ on: Bool) {
        withAnimation(.easeInOut(duration: 0.15)) { preview = on }
    }

    private var chrome: some View {
        let words = Int(wordCount(text: document.text))
        let mins = max(1, Int(ceil(Double(words) / 220.0)))
        var right = words > 0 ? "\(words) words · \(mins) min" : "empty"
        if preview { right += " · preview" }
        return HStack {
            Text("polaris")
                .font(Fonts.mono(13))
                .foregroundStyle(Tokens.quiet(dark))
            Spacer()
            Text(right)
                .font(Fonts.mono(13))
                .foregroundStyle(Tokens.quiet(dark))
        }
    }
}
