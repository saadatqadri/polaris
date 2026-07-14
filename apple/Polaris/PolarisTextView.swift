// The iPad writing surface: a UITextView we own, so smart punctuation runs
// at input time (through polaris-core) and typography is exact. Edits write
// back to the bound text; DocumentGroup persists them (autosave).
//
// This is the iOS analog of the desktop's owned editor widget — i2 wires
// smart punctuation and core-driven word count; the writing modes are later.

import SwiftUI
import UIKit

struct PolarisTextView: UIViewRepresentable {
    @Binding var text: String
    var dark: Bool
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
        // Only push external changes (e.g. a reload); never clobber the caret
        // during live typing.
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
        init(text: Binding<String>) { _text = text }

        // Smart punctuation at input time, via polaris-core.
        func textView(
            _ tv: UITextView,
            shouldChangeTextIn range: NSRange,
            replacementText replacement: String
        ) -> Bool {
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
        }
    }
}
