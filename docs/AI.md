# Writing with the door closed — Polaris's AI position

> Owner's motivation (2026-07-07): feeds are filling with machine-written
> text, and the smell is unmistakable. Polaris must not contribute to it.
> Polaris is a place to think — writing *is* thinking. AI may assist at the
> right time (reviews, notes), but it must never get into the flow of
> writing.

This document turns that motivation into binding design. It governs any
future AI work (Phase 4+). DESIGN.md principle 4 and PLAN.md principle #2
defer to it.

> Addendum (2026-07-10): these constraints are pedagogy, not just ethics.
> The owner's stated goal is that Polaris make its writer *better* — and
> an AI that rewrites your sentence improves the document while atrophying
> the writer. Critique-only means the writer does the fixing, and the
> fixing is the learning. Retype-don't-paste is deliberate practice.

## The three rules — structural, not settings

1. **No machine words in the buffer.** There is no code path that inserts
   AI output into the document. AI output lives in a separate annotation
   structure, rendered in the margin, stored beside the document (in
   `.polaris/`, with Phase 3's drafts) — never in the `.md` file. If a note
   inspires a phrasing, the writer types it. Retyping is the checkpoint
   where a machine's suggestion becomes a human's judgment. This is not a
   preference or a flag; it is an architectural invariant, like the buffer
   being a rope.

2. **Out of the flow.** Write mode contains no AI affordance whatsoever:
   no button, no shortcut, no ghost text, no ambient analysis, no
   squiggles, nothing running in the background. Not "off by default" —
   absent. AI can only be summoned by an explicit keystroke, outside the
   act of composition, on a marked draft (below). Phase 2's writing modes
   (focus, Hemingway, typewriter) are the other half of this rule: they
   protect generation-by-human; this rule keeps the machine out of it.

3. **Critique, never composition.** The editor's pass may: ask questions,
   name problems ("unclear", "unsupported", "said twice", "tone shift"),
   point at exact ranges, and propose *cuts*. It may never contain
   replacement prose. A cut is editing; an insertion is authorship. In v1,
   even accepted cuts are performed by hand — there is no accept button
   that mutates the buffer on the machine's behalf.

## The mechanism: notes on a manuscript

The right time for review is when the writer says "this is a draft" —
Phase 3's Ctrl+M. The editor's pass is only summonable on a marked draft,
never on the live buffer. That gives AI the social shape of a human
editor: you finish, you hand over a manuscript, notes come back in the
margin of *that version*.

- Annotations anchor to ranges of the marked draft (which is frozen, so
  anchors cannot drift while you keep writing).
- They render in a margin in a review surface, in `quiet`/`Mono` — chrome,
  not content.
- The pass runs once per summon. No streaming into view while you read,
  no follow-ups uninvited.

## Local-first honesty

Nothing leaves the machine except at the summoned moment, and the chrome
makes that moment visible. No background calls, no telemetry of document
text, ever. (A local model would make the pass fully offline — worth
evaluating when we get there.)

## Ruled out, permanently

Autocomplete / ghost text / tab-to-accept · "continue writing" ·
rewrite/paraphrase/tone buttons · grammar auto-fix · summarize-into-buffer
· AI templates or starters · any generation into the buffer under any
flag. If a feature requires machine words to enter the document, the
answer is no.

## Phase mapping

- **Phase 2 (now):** no AI work at all. The writing modes are the
  anti-slop design for the composition side.
- **Phase 3:** drafts create the review checkpoint. The drafts design doc
  must reserve room for annotation storage alongside snapshots.
- **Phase 4:** the editor's pass, per this document. Its own design doc
  (note vocabulary, provider/privacy, margin layout) is required before
  build.

## Open questions (decide at Phase 4 design time)

- The note vocabulary: which categories, and how few can we get away with?
- Provider: local model vs API, and how the summon moment is surfaced.
- Whether notes survive into later drafts ("addressed / still true").
- Whether a note can jump the caret to its range (probably yes — that's
  navigation, not composition).
