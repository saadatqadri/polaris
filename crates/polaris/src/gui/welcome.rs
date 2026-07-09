//! The first-run tour: shown once in the untitled buffer (config flag),
//! reopenable anytime with `polaris welcome`. It is a sample document, so
//! closing it is one keystroke and typing over it re-arms save protection.

pub const WELCOME: &str = r#"# Welcome to Polaris

One quiet page. Everything saves itself.

## The basics

- Just type — **autosave** writes about a second after your last keystroke ("● saved", top right)
- *Smart punctuation* happens as you type: -- becomes an em dash, "quotes curl", dots become an ellipsis…
- The chrome fades while you write and returns when you rest
- Close the window anytime — your words are already on disk

## Keys

| Key | Does |
|-----|------|
| Cmd+P | Preview — this page, typeset (arrows scroll; Esc returns) |
| Cmd+F | Find (Enter cycles matches) |
| Cmd+S | Save now, or name an untitled file |
| Cmd+R | Rename the file |
| Cmd+T | Light / dark |
| Cmd+Z | Undo, in word-sized steps |

## Writing modes

| Key | Mode |
|-----|------|
| Cmd+Y | Typewriter — the line you're writing holds still |
| Cmd+G | Focus — only your paragraph at full ink |
| Cmd+E | Hemingway — forward only, no deleting |
| Cmd+K | Zen — chrome hidden until summoned |
| Cmd+L | Session goal — try 500 |

## Drafts

Your document quietly keeps its history. **Cmd+M** names a version
("Draft 1 — first pass"); **Cmd+Shift+M** browses them with word-level
diffs — R restores, and even that is one Cmd+Z from undone.

> Write with the door closed, rewrite with the door open.

## Publish

**Cmd+D** deploys to Notion (`polaris config --token … --default-page …`
first). More targets are coming — write once, publish anywhere.

---

Every word in a Polaris document is typed by a person. Polaris will never
write for you.

Try **Cmd+P** right now to see this page typeset. Then delete all this and
start writing — or just close the window. (`polaris welcome` brings it back.)
"#;
