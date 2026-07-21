//! Markdown → HTML, shared by every HTML-shaped target: the standalone HTML
//! export (this file's [`HtmlTarget`]) and the Substack rich-paste. One
//! renderer, so they agree. Print-to-PDF from a browser covers PDF export —
//! no bundled webview (that stays against the design).

use crate::{Doc, Outcome, Target};
use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::Engine;
use pulldown_cmark::{html, Options, Parser};
use std::path::PathBuf;

use crate::hugo::slug;

/// The document body as an HTML fragment (no `<html>`/`<head>`), tables and
/// strikethrough enabled — the same extensions the preview renders.
pub fn body_html(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut out = String::new();
    html::push_html(&mut out, Parser::new_ext(markdown, options));
    out
}

/// The bundled faces, passed in by the binary (which owns the font bytes), so
/// an export is self-contained and on-brand without this crate vendoring
/// fonts of its own.
pub struct Fonts<'a> {
    pub newsreader_regular: &'a [u8],
    pub newsreader_italic: &'a [u8],
    pub newsreader_semibold: &'a [u8],
    pub mono_regular: &'a [u8],
}

/// A complete, self-contained HTML document: the body plus an inlined
/// stylesheet with the bundled faces base64-embedded. Opens anywhere offline;
/// Print → Save as PDF for a PDF.
pub fn standalone_html(title: &str, markdown: &str, fonts: &Fonts) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n<style>{css}</style>\n</head>\n\
         <body>\n<main class=\"page\">\n{body}</main>\n</body>\n</html>\n",
        title = escape(title),
        css = stylesheet(fonts),
        body = body_html(markdown),
    )
}

fn face(family: &str, weight: &str, style: &str, bytes: &[u8]) -> String {
    let data = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!(
        "@font-face{{font-family:'{family}';font-weight:{weight};font-style:{style};\
         src:url(data:font/ttf;base64,{data}) format('truetype');}}"
    )
}

fn stylesheet(fonts: &Fonts) -> String {
    let faces = [
        face("Newsreader", "400", "normal", fonts.newsreader_regular),
        face("Newsreader", "400", "italic", fonts.newsreader_italic),
        face("Newsreader", "600", "normal", fonts.newsreader_semibold),
        face("iA Writer Mono", "400", "normal", fonts.mono_regular),
    ]
    .join("");
    // Warm paper / ink echoing the app's light theme; a single reading measure.
    format!(
        "{faces}\
         :root{{color-scheme:light;}}\
         body{{margin:0;background:#faf8f4;color:#1c1b19;}}\
         .page{{max-width:38rem;margin:0 auto;padding:4rem 1.5rem 6rem;\
         font-family:'Newsreader',Georgia,serif;font-size:1.19rem;line-height:1.62;}}\
         h1,h2,h3,h4{{font-weight:600;line-height:1.25;margin:2.2rem 0 .6rem;}}\
         h1{{font-size:1.9rem;}}h2{{font-size:1.5rem;}}h3{{font-size:1.25rem;}}\
         p{{margin:0 0 1.1rem;}}\
         a{{color:#4e6e8e;}}\
         blockquote{{margin:1.1rem 0;padding-left:1rem;border-left:3px solid #e5e0d8;color:#57534e;}}\
         code,pre{{font-family:'iA Writer Mono',ui-monospace,monospace;font-size:.9em;}}\
         pre{{background:#f3efe8;padding:1rem;border-radius:4px;overflow-x:auto;}}\
         pre code{{font-size:.85rem;}}\
         img{{max-width:100%;height:auto;}}\
         table{{border-collapse:collapse;width:100%;margin:1.1rem 0;}}\
         th,td{{border-bottom:1px solid #e5e0d8;padding:.4rem .6rem;text-align:left;}}\
         hr{{border:none;border-top:1px solid #e5e0d8;margin:2rem 0;}}"
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Write a self-contained `<slug>.html` into a configured export directory.
pub struct HtmlTarget<'a> {
    out_dir: PathBuf,
    fonts: Fonts<'a>,
}

impl<'a> HtmlTarget<'a> {
    pub fn new(out_dir: PathBuf, fonts: Fonts<'a>) -> Self {
        Self { out_dir, fonts }
    }
}

#[async_trait]
impl Target for HtmlTarget<'_> {
    fn id(&self) -> &'static str {
        "html"
    }

    fn label(&self) -> String {
        "HTML".to_string()
    }

    async fn publish(&self, doc: Doc<'_>) -> Result<Outcome> {
        let title = doc.title().unwrap_or_else(|| "untitled".to_string());
        let path = self.out_dir.join(format!("{}.html", slug(&title)));
        let contents = standalone_html(&title, doc.markdown, &self.fonts);
        std::fs::create_dir_all(&self.out_dir)
            .with_context(|| format!("creating {}", self.out_dir.display()))?;
        std::fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))?;
        Ok(Outcome::Path(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TTF: &[u8] = b"\x00\x01\x00\x00fake-ttf";

    fn fonts() -> Fonts<'static> {
        Fonts {
            newsreader_regular: TTF,
            newsreader_italic: TTF,
            newsreader_semibold: TTF,
            mono_regular: TTF,
        }
    }

    #[test]
    fn body_html_renders_markdown() {
        let html = body_html("# Title\n\nA *word* and a [link](https://x.io).\n");
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<em>word</em>"));
        assert!(html.contains("href=\"https://x.io\""));
    }

    #[test]
    fn standalone_is_self_contained_and_escapes_the_title() {
        let out = standalone_html("A < B & C", "# Hi\n\nbody\n", &fonts());
        assert!(out.starts_with("<!doctype html>"));
        assert!(out.contains("@font-face"));
        assert!(out.contains("data:font/ttf;base64,"));
        assert!(out.contains("<title>A &lt; B &amp; C</title>"));
        assert!(out.contains("<h1>Hi</h1>"));
    }

    #[tokio::test]
    async fn html_target_writes_a_slugged_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = HtmlTarget::new(dir.path().to_path_buf(), fonts());
        let outcome = target
            .publish(Doc::new("# My Post\n\nhi\n", None))
            .await
            .unwrap();
        let Outcome::Path(path) = outcome else {
            panic!("expected a Path");
        };
        assert_eq!(path, dir.path().join("my-post.html"));
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("<h1>My Post</h1>"));
    }
}
