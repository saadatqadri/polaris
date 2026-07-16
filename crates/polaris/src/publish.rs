//! Registry glue: turn `~/.polaris.toml` into the live list of publish
//! targets. The pure adapters live in `polaris-publish`; this is where the
//! binary's config schema meets them. Design: docs/PHASE4.md.

use crate::config::Config;
use polaris_publish::{HugoTarget, NotionTarget, Target};
use std::path::PathBuf;

/// Every target the config makes available, in a stable order (Notion,
/// then Hugo). `hugo_force` overwrites an existing Hugo file; it is inert
/// for other targets.
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
            theme: None,
            onboarded: true,
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
