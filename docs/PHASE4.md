# Phase 4 — Publish Anywhere & the editing workflow (design)

> Status: APPROVED 2026-07-16. Owner added two Preview-mode features at
> approval (Part C: the reading pointer + inline notes).
> Required by PLAN §5 Phase 4. Publish layer generalizes `polaris-notion`;
> accept/reject reuses `polaris-drafts`' word diff. AI margin annotations
> are named here but stay parked behind `docs/AI.md` (own doc required).

Phase 4 is the last big MVP arc. It is **two independent halves** plus two
Preview-mode additions:

1. **Publish Anywhere** — Polaris becomes the one place everything gets
   drafted, and publishing is syndication outward. Notion today; Hugo and
   Substack are the owner's named priorities; HTML/PDF and LinkedIn follow.
2. **Accept/reject editing** — import an edited copy of a document, see a
   word-level diff, accept or reject each change. No server.
3. **Reading in Preview** (Part C) — a moving pointer so you keep your place
   (and arrow-key navigation) in preview, and **inline notes** for reviewing
   your own work. The notes are the human-first face of the AI.md margin.

Parts A and B share no code and can ship in either order. Part C rides on
the preview surface and is independent of both. This doc specs all three,
then sequences them.

---

# Part A — Publish Anywhere

## The shape of the problem

`polaris-notion` today is one hard-wired target: `NotionClient::deploy(
markdown, page_id, mode) -> Result<url>`, driven by Cmd+D and the headless
`polaris deploy` CLI. Phase 4 turns "deploy to Notion" into "publish to a
target," where Notion is just the first adapter. The design goal is that
**adding a target is adding one file**, and that the one-keystroke feel of
Cmd+D survives having many targets.

## Design decision #1: a `publish` layer of adapters, markdown in

A new workspace crate **`polaris-publish`** owns a single trait. Every
target — file-writing (Hugo), clipboard (Substack/LinkedIn), API (Notion),
export (HTML/PDF) — implements it. `polaris-notion` becomes one adapter
behind this crate (kept as its own crate for the API client weight; the
adapter is a thin wrapper).

```rust
/// What a target produces when asked to publish.
pub enum Outcome {
    /// A live URL (Notion) or a written file path (Hugo, PDF).
    Url(String),
    Path(PathBuf),
    /// The user must finish the last step by hand. `body` is already on
    /// the clipboard; `hint` tells them what to do ("Paste into a new
    /// Substack post"). This is honest about API-less targets.
    Clipboard { hint: String },
}

pub struct Doc<'a> {
    pub markdown: &'a str,
    pub title: Option<&'a str>,   // H1 if present; else the file stem
    pub source_path: Option<&'a Path>,
}

#[async_trait]
pub trait Target {
    /// Stable id used in config and the picker: "notion", "hugo", …
    fn id(&self) -> &'static str;
    /// Human label for the picker: "Notion", "Hugo — saadatqadri.com".
    fn label(&self) -> String;
    /// Pure or clipboard targets return immediately; API/file targets do I/O.
    async fn publish(&self, doc: Doc<'_>) -> Result<Outcome>;
}
```

Key properties:

- **Markdown in, platform out.** The trait never sees Polaris's buffer,
  iced, or the FFI — it takes a `&str`. Same input → same output
  (deterministic-publishing principle holds per target).
- **Clipboard and file targets share the trait with API targets.** The only
  difference is the "out" side, expressed in `Outcome`. The GUI renders the
  outcome line uniformly (URL, path, or "· copied — paste into Substack").
- **Async, but clipboard/file targets just don't await anything real.** One
  code path for Cmd+D regardless of target kind.

## Design decision #2: the target picker only appears at ≥2 targets

Cmd+D stays one keystroke. With a single configured target it fires
straight through (today's Notion behavior, unchanged). With two or more, it
summons a **chrome picker** — the same quiet list idiom as the drafts
browser, not a dialog: Up/Down, Enter to publish, Esc to cancel, first
letter jumps. A `default_target` in config makes even the multi-target case
one-key for the common path (Enter accepts the highlighted default).

The CLI mirrors this: `polaris publish --to hugo <file>` (explicit), or
`polaris publish <file>` using `default_target`. `polaris deploy` stays as a
back-compat alias for `--to notion`.

## Design decision #3: config grows a `[targets.*]` table, Notion migrates in

Today: `[notion] token / default_page`. Phase 4 generalizes without breaking
existing `~/.polaris.toml` files:

```toml
default_target = "hugo"          # optional; picker default

[notion]                         # unchanged — read as before
token = "secret_…"
default_page = "…"

[hugo]
content_dir = "~/sites/saadatqadri.com/content/posts"
# optional front-matter defaults, merged under generated title/date:
front_matter = { draft = true, author = "Saadat Qadri" }

[substack]
mode = "paste"                   # "paste" (v1) | "email" (later)
# email mode only:
# post_address = "yourpub-abc123@substack.com"
```

`NotionConfig` stays exactly as-is (back-compat); new targets are additive
`Option` sections. Absent section → target not offered.

## The targets, in priority order

### Hugo — first (owner's saadatqadri.com)

Cheapest by far: Hugo *is* markdown. The adapter:

1. Derives the **title** (H1, else file stem) and **date** (now, RFC 3339).
2. Emits **TOML front matter** (Hugo's `+++` fences match our config
   idiom), merging `front_matter` defaults under the generated keys:
   ```
   +++
   title = "The post title"
   date = 2026-07-16T09:12:00-07:00
   draft = true
   +++
   ```
3. Writes `<content_dir>/<slug>.md`, slug from the title
   (lowercased, spaces→`-`, punctuation stripped). **Collision policy:** if
   the file exists, don't clobber silently — the outcome line asks to
   confirm overwrite (a second Cmd+D), or the CLI takes `--force`.
4. Returns `Outcome::Path`. **No git automation in v1** — Polaris writes the
   file; the user commits and lets their existing deploy pipeline run.

Mermaid and images pass through untouched (Hugo renders them) — another
reason it's first: no preview-fidelity blockers.

### Substack — format-and-paste first

No official publishing API. v1 is honest:

- **`mode = "paste"` (ship first):** render markdown → Substack-friendly
  **HTML** (headings, bold/italic, links, lists, blockquotes, code; images
  as `<img>` if remote URLs, flagged as a known gap for local images), copy
  to clipboard, return `Outcome::Clipboard { hint: "Paste into a new
  Substack post" }`. Zero dependencies, always works.
- **`mode = "email"` (investigate after):** render to an email-safe HTML and
  send to the publication's post-by-email address via the user's SMTP.
  Lands as a Substack *draft* to review and send — which fits "you press
  publish, not the machine." Deferred; specced only enough to know the
  `paste` HTML renderer is the reusable part.

No unofficial-API scraping — fragile and against ToS. Stated once, here.

### HTML / PDF export — local, table stakes

Self-contained HTML (Newsreader/iA Mono inlined, the preview stylesheet)
and a PDF via the same HTML. Both `Outcome::Path`, no accounts. This reuses
the preview renderer, so it lands cheaply alongside the preview-fidelity work.

### LinkedIn — format-and-copy

Posting API is partner-gated. v1 = "format for LinkedIn (flatten headings,
convert emphasis to the Unicode tricks LinkedIn tolerates, strip unsupported
blocks) + copy to clipboard." Same `Outcome::Clipboard` path as Substack
paste. Revisit only if API access becomes realistic.

## Where the code lives

- **`polaris-publish`** (new): the `Target` trait, `Doc`, `Outcome`, the
  Hugo/Substack/HTML/PDF/LinkedIn adapters, and a `registry` that builds the
  live target list from `Config`. Pure-function testable: front-matter
  generation, slugging, markdown→HTML, given-config→target-list.
- **`polaris-notion`**: unchanged internally; gains a thin `impl Target`
  (either here or via a feature in `polaris-publish` depending on the
  dependency direction — Notion adapter wraps `NotionClient`).
- **`polaris` (GUI/CLI)**: Cmd+D consults the registry; 1 target → fire, ≥2
  → picker. The `polaris publish` CLI subcommand. Rendering `Outcome` in the
  chrome result line. Clipboard write is the GUI's job (iced clipboard), so
  clipboard targets return the *body* to the app rather than touching the
  clipboard inside the pure crate — keeps `polaris-publish` I/O-light.

  > Resolved in P1: clipboard targets return `Outcome::Clipboard { hint,
  > body }` — the rendered `body` travels in the variant, and the *app*
  > (GUI/CLI) is what places it on the clipboard, so the pure crate links no
  > clipboard backend. No clipboard targets exist yet (P2); the plumbing is
  > in place.

---

# Part B — Accept/reject editing workflow

## What it is

You wrote a draft in Polaris. Someone (an editor, a colleague, later the
summoned AI) edited a copy. You **import the edited copy**, Polaris shows a
**word-level diff**, and you walk it **accept/reject per change**, building
the final text. Draft-the-app's collaboration model, entirely local, no
server. This is also the exact surface a future AI critique pass would feed.

## Design decision #4: reuse the drafts diff, add a review state

`polaris-drafts::diff` already tokenizes and word-diffs two texts (Phase 3
uses it for draft-vs-current). Accept/reject is that diff plus a per-hunk
decision. Model the review as a list of **changes** over the diff:

```rust
pub enum ChangeKind { Insert, Delete, Replace }

pub struct Change {
    pub kind: ChangeKind,
    pub base: Range<usize>,     // word span in the original
    pub incoming: Range<usize>, // word span in the edited copy
    pub decision: Decision,     // Pending | Accepted | Rejected
}
```

Applying the review is a pure fold over the original text: for each change,
Accepted takes `incoming`, Rejected keeps `base`, Pending keeps `base` until
decided. The result is a new document string the buffer adopts through core
(one undo group — Cmd+Z reverts the whole review, consistent with restore).

**Crucially, no machine or third party writes the buffer directly.** The
diff is *proposed*; the human applies it change by change. This is the same
invariant AI.md demands, generalized to any external edit.

## Design decision #5: import is a file pick; review is a view mode

- **Import (Cmd+Shift+I?):** a file picker for the edited `.md`. Diff
  computed against the current buffer.
- **Review view mode** (like preview / drafts browser — one page, one
  focus): the document rendered with changes inline —
  - insertions in `star` (accent), deletions struck-through in `quiet`
    (matches the drafts diff palette; no new tokens);
  - the current change highlighted;
  - **J/K** move between changes, **A** accept, **R** reject, **U** undo a
    decision, **Enter** applies all decisions and returns to writing, **Esc**
    cancels the whole review (buffer untouched).
- A quiet chrome counter: "3 of 17 · 9 accepted." Reject-all / accept-all as
  Shift+A / Shift+R for the common "mostly yes" and "start clean" cases.

## Where the code lives

`polaris-drafts` gains the review model and the pure `apply(review,
original) -> String` fold (it already owns the diff — natural home). The GUI
adds the import pick, the review view mode, and the keybindings. Fully
testable without UI: diff→changes, decisions→applied text, undo grouping.

## AI.md hook (reserved, not built)

An AI critique pass (Phase 4+, own design doc per `docs/AI.md`) attaches to a
**marked draft** and emits *margin annotations and proposed cuts* — never
replacement prose, never into the buffer. Rendered, those proposals are just
another source of `Change`s flowing into this same accept/reject surface.
Building Part B first means the AI pass, if it ever ships, has nowhere to
write except the margin and the proposal list. Nothing in Phase 4 calls a
model.

---

# Part C — Reading in Preview: the pointer & inline notes

Preview mode is currently a dead end for the keyboard: you flip in with
Cmd+P, you can scroll, but you lose the caret — there's no "where am I," and
Esc back to writing lands wherever the caret was left, not where you were
reading. Two additions fix that and turn preview into a real reviewing
surface.

## Design decision #6: preview keeps a pointer, mapped to a buffer offset

Preview gets a **reading pointer** — a quiet indicator of the current
position, driven by the arrow keys:

- **Up/Down** move it by rendered block (paragraph, heading, list item);
  **Left/Right** step by sentence within the long blocks (Left/Right doing
  nothing at a block boundary rather than wrapping).
- The pointer is a **line-level marker in the margin** (a slim `star` rule at
  the left edge of the current block), not a text caret — preview is
  rendered, not editable, so a between-glyphs caret would lie about what you
  can do. `star` is legitimate here: it *is* the cursor's stand-in.
- The pointer **owns a buffer offset**. Preview already maps
  markdown→rendered spans; we keep the reverse map (rendered block → source
  range). This does double duty:
  - **Cmd+P round-trips position.** Enter preview → pointer starts at the
    caret's block. Esc/Cmd+P back to writing → caret lands at the pointer,
    not where it was. Position is preserved *both* ways (M4 preserved scroll;
    this preserves the actual cursor).
  - It's the anchor the inline notes attach to (below).
- Chrome-recedes still holds: the marker is quiet and, like the caret in
  write mode, it's the only chrome preview grows. No scrollbar-ography, no
  minimap.

Pure-testable part: the source↔rendered offset map (round-trip a caret
offset through preview and back to the same grapheme boundary).

## Design decision #7: inline notes — the human margin, AI.md-shaped

You're your own first editor. **Inline notes** let you leave a margin
comment on a span while reading in preview — "cut this," "weak transition,"
"check this number" — without touching the prose. This is deliberately the
**same margin surface AI.md reserves**: build it for the human reviewer now,
and a future summoned-AI critique pass (own doc, per AI.md) has nowhere to
put its questions *except* this margin and the Part B proposal list. No
machine words in the buffer — enforced by construction, because the buffer
has no note-writing path at all; notes live in the sidecar.

**Anchoring.** A note pins to a **source span** plus a stored **quote** of
the anchored text. On later edits the span drifts; we best-effort re-anchor
by locating the quote near the old offset. Found → note moves with the text.
Not found → the note **orphans**: kept, shown at its last block with a quiet
"· detached" tag, never silently dropped. (This is how Draft/Docs comments
survive edits; we adopt the same forgiving model.)

**Storage.** Extends the annotation store DRAFTS.md reserved:

```
<doc dir>/.polaris/<doc-stem>/notes/
  live.json                 # notes on the working document (this feature)
  <draft-id>.json           # RESERVED: notes frozen with a marked draft / AI pass
```

A note:

```json
{ "id": "n-01J…",
  "span": [1204, 1251],           // source char range at write time
  "quote": "the eventual business model",
  "body": "is this still true post-pivot?",
  "created": "2026-07-16T09:20:00Z",
  "state": "open" | "resolved" }
```

Marking a draft (Cmd+M) **freezes the open notes with it** into
`<draft-id>.json` — a draft carries the critique that was live when you
marked it, and never drifts (frozen text, per DRAFTS.md decision #1). This
is exactly the hook an AI pass would attach to.

**Interaction (in preview, all keyboard):**

- **N** — add a note at the pointer (or on the active sentence/selection).
  A quiet chrome input, like Cmd+M's; Enter saves, Esc cancels.
- Anchored spans carry a faint `star` underline; a **margin dot** sits
  beside the block. The note body shows in the **right margin** in iA Mono,
  `quiet` — the measure stays 62ch; notes live outside it, so prose column
  never reflows.
- **[ / ]** jump to the previous/next note (independent of the reading
  pointer). **Enter** on a note edits it, **X** resolves it (kept,
  greyed), **Shift+X** deletes.
- **Cmd+Shift+N** toggles notes visibility — read clean, or read with the
  critique showing. Notes never appear in write mode (preview is the
  reviewing surface; writing is for words).

**Publishing ignores notes.** Every Part A target reads the buffer markdown,
which never contained the notes — so Hugo/Substack/Notion output is clean by
construction. Nothing to strip.

## Where the code lives

- **Pointer:** GUI-only, in `preview.rs` — the rendered-block model already
  exists; add the source↔rendered map and the marker. No core/crate changes.
- **Notes:** the note model + sidecar I/O join `polaris-drafts` (it already
  owns `.polaris/<stem>/` and reserved `notes/`); re-anchoring is a pure
  function (offset + quote + new text → new offset | orphan), fully testable.
  The GUI adds the margin rendering and the keybindings.

---

# Preview fidelity (small strand, rides with Part A)

Tables ✓ and code blocks ✓ already. Still open, and settled here:

- **Inline images in preview:** render remote-URL images; local-path images
  show a labeled placeholder (a webview/asset pipeline is out of scope).
  The HTML/PDF export target reuses whatever this produces.
- **Mermaid:** stays parked — labeled source, not rendered (a JS engine or
  webview is against the design, per CLAUDE.md). Hugo and Notion pass it
  through; Substack/HTML get the labeled source. Documented limitation.

---

# Sequencing

Part A and Part B are independent. Recommended order — Part A first, because
it's the business direction and it forces the refactor everything else needs:

- **P1 — the publish layer + Hugo. ✅ SHIPPED 2026-07-16.**
  `polaris-publish` crate (`Target` trait, `Doc`, `Outcome`, Notion + Hugo
  adapters, all unit-tested); config gained `[hugo]` + `default_target`
  (`[notion]` untouched for back-compat); a binary-side registry maps config
  → `Vec<Box<dyn Target>>`; Cmd+D fires through with one target and shows a
  ✧ picker with ≥2; `polaris publish [--to id] [--force] <file>` CLI (with
  `deploy` kept as the append/replace Notion path). Two decisions made in
  build: **(a)** Notion migrated behind the trait *without* touching the
  existing `deploy` command or `[notion]` config, keeping the blast radius
  small; **(b)** Hugo strips a leading title H1 from the body — the title
  lives in front matter, and keeping the H1 would double the heading on the
  rendered page (open question about front-matter format still open — we
  shipped TOML `+++`).
- **P2 — Substack paste + LinkedIn + HTML/PDF.** The markdown→HTML renderer
  (shared by Substack paste, HTML export, and PDF), clipboard outcomes,
  LinkedIn formatting. **Done when:** Cmd+D offers a picker and each target
  produces the right outcome line.
- **P3 — accept/reject.** Review model in `polaris-drafts`, import pick,
  review view mode, keybindings. **Done when:** import an edited copy →
  walk changes → apply → Cmd+Z reverts, all from the keyboard.
- **P4 — preview fidelity + Substack email investigation + docs.** Inline
  images, the email-to-draft spike, limitations documented.

Part C slots in independently — it only touches the preview surface:

- **P5 — the reading pointer. ✅ SHIPPED 2026-07-16.** A slim accent rule in
  a reserved left gutter marks the current block; Up/Down walk it; the
  pointer owns a source byte offset (`Buffer::byte_to_char`/`char_to_byte`
  bridge the parser's byte offsets to char cursors) so Cmd+P round-trips the
  caret both ways. One walker (`preview::render_blocks`) feeds both the
  renderer and the offset map, so a pointer index means the same block in
  both. **Deferred:** Left/Right sentence-stepping within a block (design #6)
  — block-level Up/Down is the shipped core; scroll-follow is the existing
  caret-ratio approximation, not pixel-exact.
- **P6 — inline notes. ✅ SHIPPED 2026-07-16.** `NoteStore` in
  `polaris-drafts` (persist `notes/live.json`, re-anchor by quote to the
  nearest occurrence, `freeze_to(<draft-id>.json)`), notes rendered beneath
  their block in preview, N add/edit · [/] jump · x resolve · Shift+X delete
  · Cmd+Shift+N toggle, re-anchor on entering preview, Cmd+M freezes the live
  notes with the marked draft. **Scoped for v1:** notes anchor at **block**
  granularity (not sub-sentence — preview has no in-block caret), the anchor
  quote is the block's first line (resilient to edits later in the block),
  and the note body renders *beneath* its block rather than in a true right
  margin (the right-margin layout, and the iPad tap-to-reveal variant from
  open questions #8/#9, are deferred). Empty note body deletes.

P5 is a natural first pick — it's small, GUI-only, and it makes preview
navigable immediately; P6 depends on P5 (notes anchor via the same offset
map). P3 can be pulled ahead of P2 if the editing workflow is wanted sooner;
Part C can be pulled ahead of everything, since it's the cheapest win and
touches no publish or diff code.

---

# Open questions (owner input welcome)

1. **Hugo front matter: TOML `+++` or YAML `---`?** **Decided 2026-07-16:
   TOML `+++`** (shipped in P1) — matches Polaris's own config idiom. There
   is no `content/` subdir default: `content_dir` in `[hugo]` is the exact
   directory Polaris writes into, so the writer points it wherever they like
   (e.g. `.../content/posts`).
2. **Slug source:** title-derived (proposed) or the source filename? Title
   reads better in URLs; filename is predictable. Proposed: title, with the
   filename as fallback when there's no H1.
3. **Default publish target:** is Hugo the owner's day-to-day default (so
   Enter on the picker goes there), with Notion opt-in? (Proposed: yes.)
4. **Substack v1:** confirm format-and-paste ships first and email-to-draft
   is investigation-only for the MVP. (Plan says yes; confirming.)
5. **Accept/reject import trigger:** is a file picker right, or should it
   watch a conventional "edited copy" path? (Proposed: explicit pick — no
   magic folders.)
6. **Should the AI critique design doc be written now** (so Part B's `Change`
   surface is designed with it in mind) **or after Phase 4 ships?** Proposed:
   after — Part B stands alone and AI.md already constrains the shape.
7. **Pointer granularity:** block-level Up/Down + sentence-level Left/Right
   (proposed), or visual-line stepping like write mode? Blocks read more
   naturally in a rendered document; confirming.
8. **Notes on the live doc vs. only on marked drafts:** live-doc notes with
   best-effort re-anchoring (proposed — you review as you write) mean anchors
   can orphan; draft-only notes never drift but can't annotate the working
   text. Proposed: both, with live as the default surface and Cmd+M freezing
   a copy.
9. **Note margin on the iPad:** the right-margin rendering assumes desktop
   width. On iPad, do notes become a tap-to-reveal marker instead of a
   persistent margin? (Out of scope for Phase 4 desktop; flagging so P6
   doesn't hard-code a layout the iPad can't reuse.)
