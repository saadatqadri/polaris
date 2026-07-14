// A DocumentGroup-backed .md file. The plain text is the source of truth
// (local-first); iCloud/Files is Apple's sync layer, not ours.

import SwiftUI
import UniformTypeIdentifiers

struct MarkdownDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.plainText, .init(filenameExtension: "md") ?? .plainText] }
    static var writableContentTypes: [UTType] { [.plainText] }

    var text: String

    init(text: String = "") { self.text = text }

    init(configuration: ReadConfiguration) throws {
        if let data = configuration.file.regularFileContents {
            self.text = String(decoding: data, as: UTF8.self)
        } else {
            self.text = ""
        }
    }

    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: Data(text.utf8))
    }
}
