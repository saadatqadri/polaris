import SwiftUI

struct WritingView: View {
    @Binding var document: MarkdownDocument
    @Environment(\.colorScheme) private var scheme

    private var dark: Bool { scheme == .dark }

    var body: some View {
        VStack(spacing: 0) {
            chrome
                .padding(.horizontal, 24)
                .padding(.top, 14)
                .padding(.bottom, 8)

            PolarisTextView(text: $document.text, dark: dark)
                .frame(maxWidth: 620)
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 24)
        }
        .background(Tokens.bg(dark))
    }

    private var chrome: some View {
        // polaris-core computes the count through the FFI.
        let words = Int(wordCount(text: document.text))
        let mins = max(1, Int(ceil(Double(words) / 220.0)))
        return HStack {
            Text("polaris")
                .font(Fonts.mono(13))
                .foregroundStyle(Tokens.quiet(dark))
            Spacer()
            Text(words > 0 ? "\(words) words · \(mins) min" : "empty")
                .font(Fonts.mono(13))
                .foregroundStyle(Tokens.quiet(dark))
        }
    }
}
