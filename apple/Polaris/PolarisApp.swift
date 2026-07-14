// Polaris — iPad. A DocumentGroup app editing .md files; the writing
// surface is native SwiftUI, the document engine is polaris-core via FFI
// (word count, dirty state) — the iOS i1 milestone (docs/IOS.md).

import SwiftUI

@main
struct PolarisApp: App {
    var body: some Scene {
        DocumentGroup(newDocument: MarkdownDocument()) { file in
            WritingView(document: file.$document)
        }
    }
}
