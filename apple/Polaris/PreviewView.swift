// Preview mode on iPad: the same column, markdown rendered — one voice
// (Newsreader), driven by polaris-core's parser via the render_preview FFI.
// A mode switch, not a split (DESIGN.md).

import SwiftUI

struct PreviewView: View {
    let markdown: String
    var dark: Bool

    private var blocks: [PreviewBlock] { renderPreview(markdown: markdown) }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                    view(for: block)
                }
            }
            .frame(maxWidth: 620, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .center)
            .padding(.horizontal, 24)
            .padding(.top, 4)
            .padding(.bottom, 220)
        }
        .background(Tokens.bg(dark))
    }

    @ViewBuilder
    private func view(for block: PreviewBlock) -> some View {
        switch block {
        case let .heading(level, spans):
            styled(spans, semibold: true)
                .font(Fonts.writing(headingSize(level)))
                .foregroundStyle(Tokens.ink(dark))
        case let .paragraph(spans):
            styled(spans)
                .font(Fonts.writing(19))
                .lineSpacing(6)
                .foregroundStyle(Tokens.ink(dark))
        case let .listItem(_, marker, spans):
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                Text(marker)
                    .font(Fonts.writing(19))
                    .foregroundStyle(Tokens.quiet(dark))
                    .frame(width: 24, alignment: .leading)
                styled(spans)
                    .font(Fonts.writing(19))
                    .foregroundStyle(Tokens.ink(dark))
            }
        case let .quote(spans):
            HStack(spacing: 14) {
                Rectangle()
                    .fill(Tokens.quiet(dark))
                    .frame(width: 3)
                styled(spans, italic: true)
                    .font(Fonts.writing(19))
                    .foregroundStyle(Tokens.ink(dark))
            }
        case let .code(_, text):
            Text(text)
                .font(Fonts.mono(14))
                .foregroundStyle(Tokens.ink(dark))
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(14)
                .background(Tokens.quiet(dark).opacity(0.18))
                .clipShape(RoundedRectangle(cornerRadius: 4))
        case .rule:
            Rectangle().fill(Tokens.quiet(dark).opacity(0.5)).frame(height: 1)
        }
    }

    // Combine spans into one styled Text via AttributedString.
    private func styled(
        _ spans: [PreviewSpan], semibold: Bool = false, italic: Bool = false
    ) -> Text {
        var attr = AttributedString()
        for s in spans {
            var run = AttributedString(s.text)
            if s.code {
                run.font = Fonts.mono(15)
            } else if s.bold || semibold {
                run.font = Fonts.writing(19).weight(.semibold)
            } else if s.italic || italic {
                run.font = Fonts.writing(19).italic()
            }
            attr.append(run)
        }
        return Text(attr)
    }

    private func headingSize(_ level: UInt8) -> CGFloat {
        switch level {
        case 1: return 27
        case 2: return 22.5
        default: return 19.5
        }
    }
}
