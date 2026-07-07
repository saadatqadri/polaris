# Spike report: the custom editor widget (Phase 2)

**Verdict: viable — proceed.** `polaris spike [file]` opens a window where
every glyph is laid out by our own iced widget (`gui/spike.rs`, ~550
lines) over `polaris-core::Document`. No `text_editor` involved.

## What the spike proves (try it)

| Capability | Status | How |
|---|---|---|
| Typewriter scrolling (Cmd+Y) | ✅ | The widget owns the scroll offset; the caret's visual row is held at 45% of the viewport. Impossible with `text_editor` (internal scroll). |
| Focus dimming (Cmd+G) | ✅ | Per-paragraph ink alpha at draw time — the caret's paragraph at full ink, the rest at 30%. |
| Markdown-quiet marks | ✅ | Per-line span styling: `#`/`>`/`-` prefixes render in `quiet`, heading text semibold, quotes italic. Inline `**`/`*` marks need a small line-parser producing more spans — same machinery, no new capability. |
| Core-driven caret & editing | ✅ | Typing, Enter, Backspace/Delete, arrows (word-jump via Alt), Home/End, click-to-place, undo/redo — all `Document` calls; the widget renders `doc.cursor()`. Grapheme correctness comes from core for free. |

## Implementation notes

- **One `Paragraph` per buffer line**, cached in widget state, rebuilt when
  text version or wrap width changes. Prose paragraphs are single buffer
  lines, so incremental relayout is naturally cheap.
- **Caret geometry via a probe layout**: lay out the text up to the caret
  at the same width; row = probe height / line height, x =
  `grapheme_position(last_row, MAX)`. One extra paragraph layout per frame
  — negligible. (`Paragraph::grapheme_position` indexes by *visual run*,
  which is why the probe is the robust path.)
- Needs the `advanced` iced feature (Widget/renderer traits).

## Known costs to reach parity (the real Phase 2 work)

Ordered by risk:

1. **IME / dead keys** — the spike inserts `KeyPressed.text` only: composed
   input (é via option-e, CJK, dictation) needs `Event::InputMethod`
   handling + preedit rendering. iced 0.14 has the events; `text_editor`'s
   handling is the reference. This is the largest single item.
2. **Selection rendering** — core owns selection ranges already; drawing
   needs per-run rects (probe technique extends to this, or
   `span_bounds`). Mouse drag-selection + double-click word select.
3. **Clipboard** — copy/cut/paste via the `Clipboard` handle in
   `Widget::update` (plumbing, not risk).
4. **Caret blink + smooth scroll** — `shell.request_redraw` scheduling;
   cosmetic.
5. **Focus** — spike assumes always-focused (true for a single-widget
   window); needs the focus protocol if overlays keep `text_input`.

## Recommendation

Promote the spike into `gui/editor.rs` behind the real app incrementally:
land IME + selection + clipboard first (parity), then flip the main window
from the `text_editor` shim to the widget, then delete `apply_diff` — the
Document becomes the single source of truth with no sync layer at all.
Typewriter/focus/zen then become Phase 2 feature flags on the widget
rather than new machinery.
