//! Registry glue: turn `~/.polaris.toml` into the live list of publish
//! targets. The pure adapters live in `polaris-publish`; this is where the
//! binary's config schema meets them. Design: docs/PHASE4.md.

use crate::config::Config;
use crate::gui::fonts;
use polaris_publish::{
    Fonts, HtmlTarget, HugoTarget, LinkedinTarget, NotionTarget, SubstackTarget, Target,
};
use std::path::PathBuf;

/// The bundled faces, borrowed from the binary's embedded byte constants —
/// `'static`, so an `HtmlTarget` built from them is a `'static` trait object.
fn bundled_fonts() -> Fonts<'static> {
    Fonts {
        newsreader_regular: fonts::WRITING_REGULAR_BYTES,
        newsreader_italic: fonts::WRITING_ITALIC_BYTES,
        newsreader_semibold: fonts::WRITING_SEMIBOLD_BYTES,
        mono_regular: fonts::MONO_REGULAR_BYTES,
    }
}

/// Every target the config makes available, in a stable order (Notion, Hugo,
/// HTML, Substack, LinkedIn). `hugo_force` overwrites an existing Hugo file;
/// it is inert for other targets.
pub fn available(config: &Config, hugo_force: bool) -> Vec<Box<dyn Target>> {
    let mut targets: Vec<Box<dyn Target>> = Vec::new();

    if let (Some(token), Some(page)) = (&config.notion.token, &config.notion.default_page) {
        targets.push(Box::new(NotionTarget::new(token.clone(), page.clone())));
    }

    if let Some(hugo) = &config.hugo {
        let dir = expand_tilde(&hugo.content_dir);
        let defaults = hugo.front_matter.clone().unwrap_or_default();
        targets.push(Box::new(
            HugoTarget::new(dir, defaults).with_force(hugo_force),
        ));
    }

    if let Some(html) = &config.html {
        let dir = expand_tilde(&html.out_dir);
        targets.push(Box::new(HtmlTarget::new(dir, bundled_fonts())));
    }

    if config.substack.is_some() {
        targets.push(Box::new(SubstackTarget));
    }

    if config.linkedin.is_some() {
        targets.push(Box::new(LinkedinTarget));
    }

    targets
}

/// The id of the target Cmd+D / `publish` should default to, given the
/// available list: the configured `default_target` if it is available, else
/// the first available, else `None` (nothing configured).
pub fn default_id(config: &Config, targets: &[Box<dyn Target>]) -> Option<String> {
    if let Some(want) = &config.default_target {
        if targets.iter().any(|t| t.id() == want) {
            return Some(want.clone());
        }
    }
    targets.first().map(|t| t.id().to_string())
}

/// Place a clipboard target's rendered content on the system clipboard: the
/// rich `html` flavour when present (so it pastes formatted, e.g. Substack),
/// else plain `text`. The `text` is always the alt/fallback.
pub fn copy_to_clipboard(html: Option<&str>, text: &str) -> anyhow::Result<()> {
    let mut clipboard = arboard::Clipboard::new()?;
    match html {
        Some(html) => clipboard.set_html(html, Some(text))?,
        None => clipboard.set_text(text.to_string())?,
    }
    Ok(())
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HugoConfig, NotionConfig};

    fn cfg(notion: bool, hugo: bool, default_target: Option<&str>) -> Config {
        Config {
            notion: if notion {
                NotionConfig {
                    token: Some("secret".into()),
                    default_page: Some("abc123".into()),
                }
            } else {
                NotionConfig::default()
            },
            hugo: hugo.then(|| HugoConfig {
                content_dir: "~/site/content".into(),
                front_matter: None,
            }),
            default_target: default_target.map(String::from),
            ..Config::default()
        }
    }

    #[test]
    fn only_configured_targets_are_offered() {
        assert!(available(&cfg(false, false, None), false).is_empty());
        let ids: Vec<_> = available(&cfg(true, true, None), false)
            .iter()
            .map(|t| t.id())
            .collect();
        assert_eq!(ids, vec!["notion", "hugo"]);
    }

    #[test]
    fn clipboard_and_html_targets_are_offered_in_order() {
        use crate::config::{HtmlConfig, LinkedinConfig, SubstackConfig};
        let config = Config {
            hugo: Some(HugoConfig {
                content_dir: "~/site".into(),
                front_matter: None,
            }),
            html: Some(HtmlConfig {
                out_dir: "~/exports".into(),
            }),
            substack: Some(SubstackConfig {}),
            linkedin: Some(LinkedinConfig {}),
            ..Config::default()
        };
        let ids: Vec<_> = available(&config, false).iter().map(|t| t.id()).collect();
        assert_eq!(ids, vec!["hugo", "html", "substack", "linkedin"]);
    }

    #[test]
    fn default_id_honours_config_then_falls_back_to_first() {
        let both = cfg(true, true, Some("hugo"));
        let targets = available(&both, false);
        assert_eq!(default_id(&both, &targets).as_deref(), Some("hugo"));

        // Unavailable default falls back to the first available target.
        let stale = cfg(true, false, Some("hugo"));
        let targets = available(&stale, false);
        assert_eq!(default_id(&stale, &targets).as_deref(), Some("notion"));

        // Nothing configured → no default.
        let empty = cfg(false, false, None);
        let targets = available(&empty, false);
        assert_eq!(default_id(&empty, &targets), None);
    }
}
