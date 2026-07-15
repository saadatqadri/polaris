// The iPad writing surface: a UITextView we own, so smart punctuation runs
// at input time (through polaris-core) and typography is exact. Edits write
// back to the bound text; DocumentGroup persists them (autosave).
//
// Writing modes that don't need custom rendering live here: Hemingway
// (block deletions) and typewriter scrolling (hold the caret line steady).
// Focus dimming needs per-paragraph styling — a later, larger step.

import SwiftUI
import UIKit

struct PolarisTextView: UIViewRepresentable {
    @Binding var text: String
    var dark: Bool
    var hemingway: Bool = false
    var typewriter: Bool = false
    var autofocus: Bool = false

    func makeUIView(context: Context) -> UITextView {
        let tv = UITextView()
        tv.delegate = context.coordinator
        tv.backgroundColor = .clear
        tv.textContainerInset = UIEdgeInsets(top: 4, left: 0, bottom: 220, right: 0)
        tv.textContainer.lineFragmentPadding = 0
        tv.alwaysBounceVertical = true
        tv.keyboardDismissMode = .interactive
        tv.autocorrectionType = .default
        applyStyle(tv)
        tv.text = text
        if autofocus {
            DispatchQueue.main.async { tv.becomeFirstResponder() }
        }
        return tv
    }

    func updateUIView(_ tv: UITextView, context: Context) {
        context.coordinator.hemingway = hemingway
        context.coordinator.typewriter = typewriter
        if tv.text != text && !tv.isFirstResponder {
            tv.text = text
        }
        applyStyle(tv)
    }

    private func applyStyle(_ tv: UITextView) {
        let ink = UIColor(Tokens.ink(dark))
        tv.textColor = ink
        tv.tintColor = UIColor(Tokens.star(dark))
        tv.font = UIFont(name: "Newsreader16pt-Regular", size: 19)
            ?? .systemFont(ofSize: 19)
        tv.typingAttributes = [
            .font: tv.font!,
            .foregroundColor: ink,
        ]
    }

    func makeCoordinator() -> Coordinator { Coordinator(text: $text) }

    final class Coordinator: NSObject, UITextViewDelegate {
        @Binding var text: String
        var hemingway = false
        var typewriter = false
        init(text: Binding<String>) { _text = text }

        func textView(
            _ tv: UITextView,
            shouldChangeTextIn range: NSRange,
            replacementText replacement: String
        ) -> Bool {
            // Hemingway: forward only — block deletions and range replacements.
            if hemingway && replacement.isEmpty {
                return false
            }
            // Smart punctuation at input time, via polaris-core.
            guard replacement.count == 1 else { return true }
            let full = tv.text as NSString
            let before = full.substring(to: range.location)
            guard let sub = smartSubstitution(before: before, typed: replacement) else {
                return true
            }
            let del = Int(sub.deleteBefore)
            let start = max(0, range.location - del)
            let editRange = NSRange(location: start, length: del + range.length)
            guard
                let from = tv.position(from: tv.beginningOfDocument, offset: editRange.location),
                let to = tv.position(from: from, offset: editRange.length),
                let uiRange = tv.textRange(from: from, to: to)
            else { return true }
            tv.replace(uiRange, withText: sub.insert)
            text = tv.text
            return false
        }

        func textViewDidChange(_ tv: UITextView) {
            text = tv.text
            if typewriter { holdCaret(tv) }
        }

        func textViewDidChangeSelection(_ tv: UITextView) {
            if typewriter { holdCaret(tv) }
        }

        // Typewriter scrolling: keep the caret line at ~45% of the viewport.
        private func holdCaret(_ tv: UITextView) {
            guard let range = tv.selectedTextRange else { return }
            let caret = tv.caretRect(for: range.end)
            guard caret.origin.y.isFinite else { return }
            let target = tv.bounds.height * 0.45
            let desired = caret.midY - target
            let maxOffset = max(-tv.adjustedContentInset.top,
                                tv.contentSize.height - tv.bounds.height
                                    + tv.adjustedContentInset.bottom)
            let y = min(max(desired, -tv.adjustedContentInset.top), maxOffset)
            if abs(tv.contentOffset.y - y) > 0.5 {
                tv.setContentOffset(CGPoint(x: 0, y: y), animated: false)
            }
        }
    }
}
