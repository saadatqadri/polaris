# Drafts — writer-friendly version control (Phase 3 design)

> Status: DRAFT — awaiting owner approval before build.
> Required by PLAN §5 Phase 3; annotation storage reserved per docs/AI.md.

## What it is

Draft-the-app's killer feature, local-first: your document quietly keeps
its own history. **Cmd+M marks a draft** with a name ("Draft 3 — after
Sarah's notes"); a drafts browser shows named versions with word-level
diffs and one-key restore. Autosnapshots catch what you forgot to mark.
The writer never sees version-control vocabulary — no commit, no branch,
no merge, no HEAD.

## Design decision #1: snapshots, not git

PLAN §5 sketched "invisible git under `.polaris/`". This doc revises that:
**a content-addressed snapshot store, no git.** Reasons:

- Prose documents are tiny (a novel chapter is ~50KB). Full snapshots,
  zstd-compressed and deduplicated by content hash, cost nothing. Git's
  packfile machinery solves a problem we don't have.
- The history is **merge-free and linear by design** (decision #3) — git's
  entire value is branching/merging we will never expose.
- Repo-inside-repo pain (the doc lives in the user's real git repo)
  disappears: `.polaris/` self-ignores with a one-line `.gitignore`.
- No libgit2/gitoxide dependency; the store is ~200 lines of Rust we
  fully understand.
- "The writer never sees git vocabulary" is easiest when there is no git.

Revisit only if Phase 4 collaboration genuinely needs git interchange.

## Data model

Sidecar directory next to the document (history travels with the folder):

```
<doc dir>/.polaris/
  .gitignore                    # contains "*" — self-ignoring inside real repos
  <doc-stem>/
    manifest.json               # the version list, newest last
    objects/<sha256-16>.zst     # content-addressed snapshot bodies
    notes/<draft-id>.json       # RESERVED: Phase 4 margin annotations
```

Manifest entry:

```json
{ "id": "d-01J...",             // ulid-ish, sortable
  "kind": "marked" | "auto",
  "name": "Draft 3 — after Sarah's notes",   // marked only
  "created": "2026-07-08T14:31:02Z",
  "object": "a1b2c3…",          // content hash of the snapshot
  "words": 1742 }
```

- **Content-addressed**: identical text → same object; marking twice
  without edits stores one body.
- **Linear**: order in the manifest is the history. No parents, no DAG.
- **Renames**: `Cmd+R` goes through `Document::rename`, which will migrate
  `<doc-stem>/` alongside the file. Renamed outside Polaris → history
  orphans under the old stem (kept, listed as "previous name" if we can
  match by content hash; otherwise inert — documented limitation).

## Snapshot policy

- **Marked drafts (Cmd+M)**: kept forever. Never pruned.
- **Auto snapshots**: one at file-open (the "when I sat down" baseline),
  then at most one per 10 minutes of active editing, taken on the autosave
  path (no extra writes when idle). Pruning: keep the last 50 autos or 7
  days, whichever is more. Marked drafts are exempt.
- **Restore snapshots**: restoring first auto-snapshots the current state
  ("before restoring Draft 2") — restore can never lose words.

## UX (all in the column and chrome — no dialogs, no panels)

- **Cmd+M — mark a draft.** Chrome input, prefilled "Draft {n}". Enter
  saves; Esc cancels. Quiet "· draft marked" in the chrome for a beat.
- **Cmd+Shift+M — drafts browser.** A view mode like preview (one page,
  one focus): marked drafts as a list in the writing face — name, age
  ("2 days ago"), word count and delta ("+312") — autos beneath them,
  quieter. Up/Down to move, Enter to view, Esc back to writing.
- **Viewing a draft**: read-only, same column, with a **word-level diff
  against the current text**: words only in the draft shown struck-through
  in `quiet`; words only in current shown… nothing (you're looking at the
  draft). A second key (Tab?) flips diff direction. Diff via the `similar`
  crate's word tokenizer.
- **R — restore** (while viewing): auto-snapshot current, replace the
  buffer through core (one undo group — Cmd+Z works), back to writing.
- Diff emphasis uses `quiet` + strikethrough only. `star` stays reserved
  for cursor/selection; if quiet-only proves illegible in practice, the
  token question goes back to DESIGN.md rather than being decided ad hoc.

## Where the code lives

New workspace crate **`polaris-drafts`** (no UI, no iced): store,
manifest, snapshot policy, word-diff. `polaris-core` stays lean; the GUI
adds the two keybindings, the browser view mode, and rename migration.
Everything pure-function testable: store round-trips, dedup, pruning,
diff output, restore-snapshots-first.

## AI.md hook (reserved, not built)

Phase 4's editor pass attaches to a **marked draft id**; its notes land in
`notes/<draft-id>.json`. Drafts are frozen, so annotations never drift.
Nothing in Phase 3 calls any model.

## Milestones

- **D1 — the store** (`polaris-drafts`): objects, manifest, policy,
  pruning, word-diff. Fully tested, no UI.
- **D2 — mark & auto**: Cmd+M overlay, auto-snapshots on the autosave
  path, restore-snapshot-first plumbing in core/app.
- **D3 — the browser**: view mode, list, draft view with word-diff,
  restore. Done when: mark → write more → view diff → restore → undo, all
  without leaving the keyboard.
- **D4 — housekeeping**: rename migration, pruning on open, orphan
  handling, docs.

## Open questions (owner input welcome)

1. Auto-snapshot cadence: is 10 minutes right? (Every autosave is too
   many; sessions-only loses too much.)
2. Drafts browser reachable in zen mode — should Cmd+Shift+M summon
   chrome, or is the browser itself "summoning"? (Proposed: it's a view
   mode; zen doesn't apply inside it.)
3. Should the delta in the list ("+312 words") compare to the previous
   draft or to current? (Proposed: previous draft.)
4. `.polaris/` visibility: some writers will notice the folder. Name it
   `.polaris` (hidden, proposed) — acceptable?
