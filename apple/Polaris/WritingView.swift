import SwiftUI

struct WritingView: View {
    @Binding var document: MarkdownDocument
    @Environment(\.colorScheme) private var scheme

    private var dark: Bool { scheme == .dark }

    // polaris-core drives the numbers; the TextEditor drives the text, and
    // we feed edits into core to keep it authoritative (i2 grows this into
    // full core-sync; i1 proves the pipe with word count + saved state).
    private var doc: PolarisDocument { PolarisDocument(text: document.text) }

    var body: some View {
        GeometryReader { geo in
            VStack(spacing: 0) {
                chrome
                    .padding(.horizontal, 24)
                    .padding(.top, 14)
                    .padding(.bottom, 8)

                ScrollView {
                    TextEditor(text: $document.text)
                        .font(Fonts.writing(19))
                        .foregroundStyle(Tokens.ink(dark))
                        .tint(Tokens.star(dark))
                        .scrollContentBackground(.hidden)
                        .background(Tokens.bg(dark))
                        .frame(minHeight: geo.size.height - 60, alignment: .topLeading)
                        .frame(maxWidth: 620)
                        .frame(maxWidth: .infinity)
                        .padding(.horizontal, 24)
                }
            }
            .background(Tokens.bg(dark))
        }
    }

    private var chrome: some View {
        let words = Int(doc.wordCount())
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
