// A real Swift exercise of polaris-core through the uniffi bridge — the i0
// proof that the FFI works end to end. Runs on the host via check-host.sh;
// the same calls are what the iOS app will make.

let doc = PolarisDocument(text: "héllo")
doc.setCursor(pos: doc.lenChars())
doc.insert(text: " 👋 world")
assert(doc.text() == "héllo 👋 world", "text: \(doc.text())")
assert(doc.wordCount() == 2, "words: \(doc.wordCount())")

_ = doc.undo()
assert(doc.text() == "héllo", "after undo: \(doc.text())")
_ = doc.redo()
assert(doc.text() == "héllo 👋 world", "after redo: \(doc.text())")

doc.newline()
doc.insert(text: "line two")
assert(doc.text() == "héllo 👋 world\nline two")

print("Swift <-> polaris-core OK")
