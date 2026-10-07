use serde::Deserialize;
use std::{path::PathBuf, str::FromStr};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Renderer {
    Mathjax,
    Ratex,
}
impl Renderer {
    pub fn name(self) -> &'static str {
        match self {
            Self::Mathjax => "mathjax",
            Self::Ratex => "ratex",
        }
    }
}
impl FromStr for Renderer {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "mathjax" => Ok(Self::Mathjax),
            "ratex" => Ok(Self::Ratex),
            _ => Err(format!("unknown renderer {s:?}; choose ratex or mathjax")),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    #[default]
    Compact,
    Compatibility,
}
impl FromStr for Layout {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "compact" => Ok(Self::Compact),
            "compatibility" => Ok(Self::Compatibility),
            _ => Err(format!(
                "unknown layout {s:?}; choose compact or compatibility"
            )),
        }
    }
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    renderer: Option<Renderer>,
    layout: Option<Layout>,
}
pub struct Options {
    pub renderer: Renderer,
    pub layout: Layout,
    pub command: Vec<String>,
}
pub fn resolve(
    mut args: Vec<String>,
    env: Option<&str>,
    layout_env: Option<&str>,
    file: &str,
) -> Result<Options, String> {
    let conf: FileConfig =
        toml::from_str(file).map_err(|e| format!("invalid TermiTex config: {e}"))?;
    // RaTeX is the default after the representative native/PTY compatibility gate.
    let mut renderer = env
        .map(str::parse)
        .transpose()?
        .or(conf.renderer)
        .unwrap_or(Renderer::Ratex);
    let mut layout = layout_env
        .map(str::parse)
        .transpose()?
        .or(conf.layout)
        .unwrap_or_default();
    while let Some(arg) = args.first() {
        if arg == "--" {
            args.remove(0);
            break;
        }
        if arg == "--layout" {
            args.remove(0);
            if args.is_empty() {
                return Err("--layout requires compact or compatibility".into());
            }
            layout = args.remove(0).parse()?;
        } else if let Some(name) = arg.strip_prefix("--layout=") {
            layout = name.parse()?;
            args.remove(0);
        } else if arg == "--renderer" {
            args.remove(0);
            if args.is_empty() {
                return Err("--renderer requires ratex or mathjax".into());
            }
            renderer = args.remove(0).parse()?;
        } else if let Some(name) = arg.strip_prefix("--renderer=") {
            renderer = name.parse()?;
            args.remove(0);
        } else {
            break;
        }
    }
    if args.is_empty() {
        args = vec![
            "codex".into(),
            "-c".into(),
            "tui.rendering.math=false".into(),
        ];
    }
    Ok(Options {
        renderer,
        layout,
        command: args,
    })
}
pub fn load(args: Vec<String>) -> Result<Options, Box<dyn std::error::Error>> {
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    let file = match root {
        Some(root) => match std::fs::read_to_string(root.join("termitex/config.toml")) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        },
        None => String::new(),
    };
    Ok(resolve(
        args,
        std::env::var("TERMITEX_RENDERER").ok().as_deref(),
        std::env::var("TERMITEX_LAYOUT").ok().as_deref(),
        &file,
    )?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout_precedence_and_validation() {
        assert_eq!(
            resolve(vec![], None, None, "").unwrap().layout,
            Layout::Compact
        );
        assert_eq!(
            resolve(vec![], None, None, "layout='compatibility'")
                .unwrap()
                .layout,
            Layout::Compatibility
        );
        assert_eq!(
            resolve(vec![], None, Some("compatibility"), "layout='compact'")
                .unwrap()
                .layout,
            Layout::Compatibility
        );
        let options = resolve(
            vec![
                "--layout=compact".into(),
                "--".into(),
                "app".into(),
                "--layout=child".into(),
            ],
            None,
            Some("compatibility"),
            "",
        )
        .unwrap();
        assert_eq!(options.layout, Layout::Compact);
        assert_eq!(options.command, ["app", "--layout=child"]);
        assert_eq!(
            resolve(
                vec!["--layout".into(), "compatibility".into()],
                None,
                None,
                ""
            )
            .unwrap()
            .layout,
            Layout::Compatibility
        );
        assert!(resolve(vec!["--layout".into()], None, None, "").is_err());
        assert!(resolve(vec![], None, None, "layout='typo'").is_err());
        assert!(resolve(vec![], None, Some("typo"), "").is_err());
    }
    #[test]
    fn precedence_and_passthrough() {
        let options = resolve(
            vec![
                "--renderer".into(),
                "ratex".into(),
                "--".into(),
                "codex".into(),
                "--renderer=not-ours".into(),
            ],
            Some("mathjax"),
            None,
            "renderer='mathjax'",
        )
        .unwrap();
        assert_eq!(options.renderer, Renderer::Ratex);
        assert_eq!(options.command, ["codex", "--renderer=not-ours"]);
        assert_eq!(
            resolve(vec![], Some("mathjax"), None, "renderer='ratex'")
                .unwrap()
                .renderer,
            Renderer::Mathjax
        );
        assert_eq!(
            resolve(vec![], None, None, "renderer='ratex'")
                .unwrap()
                .renderer,
            Renderer::Ratex
        );
    }
    #[test]
    fn native_default() {
        let options = resolve(vec![], None, None, "").unwrap();
        assert_eq!(options.renderer, Renderer::Ratex);
        assert_eq!(options.command, ["codex", "-c", "tui.rendering.math=false"]);
    }
    #[test]
    fn reject_typos() {
        assert!(resolve(vec!["--renderer".into()], None, None, "").is_err());
        assert!(resolve(vec![], None, None, "renderer='typo'").is_err());
        assert!(resolve(vec![], None, None, "rendrer='ratex'").is_err());
    }
}
