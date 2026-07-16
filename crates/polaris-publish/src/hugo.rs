//! The Hugo adapter — the cheapest target, because Hugo *is* markdown.
//!
//! Derive a title and date, emit TOML front matter (Hugo's `+++` fences
//! match Polaris's own config idiom), and write `<content_dir>/<slug>.md`.
//! No git automation: Polaris writes the file, the writer commits. Mermaid
//! and images pass straight through — Hugo renders them.

use crate::{Doc, Outcome, Target};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Local};
use std::path::PathBuf;

pub struct HugoTarget {
    content_dir: PathBuf,
    /// Optional front-matter keys merged *under* the generated title/date —
    /// e.g. `draft = true`, `author = "…"`. Generated keys always win.
    front_matter_defaults: toml::Table,
    /// When false, refuse to overwrite an existing file (the caller retries
    /// with force after confirming). When true, overwrite.
    force: bool,
}

impl HugoTarget {
    pub fn new(content_dir: PathBuf, front_matter_defaults: toml::Table) -> Self {
        Self {
            content_dir,
            front_matter_defaults,
            force: false,
        }
    }

    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }
}

#[async_trait]
impl Target for HugoTarget {
    fn id(&self) -> &'static str {
        "hugo"
    }

    fn label(&self) -> String {
        "Hugo".to_string()
    }

    async fn publish(&self, doc: Doc<'_>) -> Result<Outcome> {
        let title = doc.title().unwrap_or_else(|| "untitled".to_string());
        let path = self.content_dir.join(format!("{}.md", slug(&title)));

        if path.exists() && !self.force {
            bail!(
                "{} already exists — publish again to overwrite (or --force)",
                path.display()
            );
        }

        let front = front_matter(&title, &Local::now(), &self.front_matter_defaults);
        let body = strip_leading_title(doc.markdown, &title);
        let contents = format!("{front}\n{body}");

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))?;

        Ok(Outcome::Path(path))
    }
}

/// A URL-safe slug from a title: lowercase, alphanumerics kept, every other
/// run collapsed to a single `-`, no leading/trailing dash. Unicode letters
/// survive (Hugo handles them); empty input yields `"untitled"`.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            if pending_dash {
                out.push('-');
                pending_dash = false;
            }
            out.extend(ch.to_lowercase());
        } else if !out.is_empty() {
            pending_dash = true;
        }
    }
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}

/// Drop the body's opening `# title` when it is the very heading we lifted
/// into front matter — Hugo renders the title from front matter, so keeping
/// it duplicates the heading on the page. Only a *leading* H1 (the first
/// non-blank line) is removed; an H1 deeper in the document is left alone,
/// and a title taken from the filename never matches, so nothing is dropped.
fn strip_leading_title(markdown: &str, title: &str) -> String {
    let mut lines = markdown.lines().peekable();

    // Skip any leading blank lines to reach the first real line.
    while lines.peek().is_some_and(|l| l.trim().is_empty()) {
        lines.next();
    }

    let is_our_title = lines
        .peek()
        .and_then(|l| l.trim_start().strip_prefix("# "))
        .map(str::trim)
        == Some(title);
    if !is_our_title {
        return markdown.to_string();
    }

    lines.next(); // the H1 itself
    if lines.peek().is_some_and(|l| l.trim().is_empty()) {
        lines.next(); // one blank line after the title
    }
    let rest: Vec<&str> = lines.collect();
    if rest.is_empty() {
        String::new()
    } else {
        format!("{}\n", rest.join("\n"))
    }
}

/// TOML front matter fenced with `+++`. `title` and `date` are generated;
/// `defaults` fills in anything else (draft flag, author) but never
/// overrides the generated keys.
pub fn front_matter(title: &str, date: &DateTime<Local>, defaults: &toml::Table) -> String {
    let mut table = toml::Table::new();
    table.insert("title".to_string(), toml::Value::String(title.to_string()));
    table.insert("date".to_string(), toml::Value::String(date.to_rfc3339()));
    for (key, value) in defaults {
        table.entry(key.clone()).or_insert_with(|| value.clone());
    }
    // `toml::to_string` on a Table can only fail on unrepresentable values,
    // which String/scalar defaults never are.
    let body = toml::to_string(&table).unwrap_or_default();
    format!("+++\n{body}+++\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Doc;

    #[test]
    fn slugs_lowercase_and_hyphenate() {
        assert_eq!(slug("The Post Title"), "the-post-title");
        assert_eq!(
            slug("Write Once, Publish Anywhere!"),
            "write-once-publish-anywhere"
        );
        assert_eq!(slug("  spaced  out  "), "spaced-out");
        assert_eq!(slug("café résumé"), "café-résumé");
        assert_eq!(slug("---"), "untitled");
    }

    #[test]
    fn front_matter_is_fenced_and_carries_generated_keys() {
        // Parse-then-render round-trips the instant through the machine's
        // local offset, so assert the emitted date equals what rfc3339 gives
        // for this DateTime<Local> rather than a hard-coded offset.
        let date = DateTime::parse_from_rfc3339("2026-07-16T09:12:00-07:00")
            .unwrap()
            .with_timezone(&Local);
        let fm = front_matter("Hello World", &date, &toml::Table::new());
        assert!(fm.starts_with("+++\n"));
        assert!(fm.trim_end().ends_with("+++"));
        assert!(fm.contains("title = \"Hello World\""));
        assert!(fm.contains(&format!("date = \"{}\"", date.to_rfc3339())));
    }

    #[test]
    fn defaults_fill_in_but_never_override_generated_keys() {
        let mut defaults = toml::Table::new();
        defaults.insert("draft".to_string(), toml::Value::Boolean(true));
        // A stray "title" default must NOT clobber the real title.
        defaults.insert(
            "title".to_string(),
            toml::Value::String("WRONG".to_string()),
        );
        let fm = front_matter("Right", &Local::now(), &defaults);
        assert!(fm.contains("draft = true"));
        assert!(fm.contains("title = \"Right\""));
        assert!(!fm.contains("WRONG"));
    }

    #[tokio::test]
    async fn writes_a_front_mattered_file_named_by_slug() {
        let dir = tempfile::tempdir().unwrap();
        let target = HugoTarget::new(dir.path().to_path_buf(), toml::Table::new());
        let md = "# My First Post\n\nHello, world.\n";

        let outcome = target.publish(Doc::new(md, None)).await.unwrap();
        let Outcome::Path(path) = outcome else {
            panic!("expected a Path outcome");
        };
        assert_eq!(path, dir.path().join("my-first-post.md"));

        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.starts_with("+++\n"));
        assert!(written.contains("title = \"My First Post\""));
        assert!(written.contains("Hello, world."));
        // The title H1 belongs to front matter, not the body — no duplicate.
        assert!(!written.contains("# My First Post"));
    }

    #[test]
    fn strip_leading_title_only_touches_the_matching_leading_h1() {
        // Leading title (with a blank line before it) is removed.
        assert_eq!(
            strip_leading_title("\n# Title\n\nbody\n", "Title"),
            "body\n"
        );
        // A non-matching heading stays.
        assert_eq!(
            strip_leading_title("# Other\n\nbody\n", "Title"),
            "# Other\n\nbody\n"
        );
        // An H1 that isn't the leading line stays (title came from elsewhere).
        assert_eq!(
            strip_leading_title("intro\n\n# Title\n", "Title"),
            "intro\n\n# Title\n"
        );
    }

    #[tokio::test]
    async fn refuses_to_overwrite_unless_forced() {
        let dir = tempfile::tempdir().unwrap();
        let md = "# Dup\n\nbody\n";

        HugoTarget::new(dir.path().to_path_buf(), toml::Table::new())
            .publish(Doc::new(md, None))
            .await
            .unwrap();

        // Second write without force is refused.
        let err = HugoTarget::new(dir.path().to_path_buf(), toml::Table::new())
            .publish(Doc::new(md, None))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("already exists"));

        // With force it goes through.
        HugoTarget::new(dir.path().to_path_buf(), toml::Table::new())
            .with_force(true)
            .publish(Doc::new(md, None))
            .await
            .unwrap();
    }
}
