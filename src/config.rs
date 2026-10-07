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
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    renderer: Option<Renderer>,
}
pub struct Options {
    pub renderer: Renderer,
    pub command: Vec<String>,
}
pub fn resolve(mut args: Vec<String>, env: Option<&str>, file: &str) -> Result<Options, String> {
    let conf: FileConfig =
        toml::from_str(file).map_err(|e| format!("invalid TermiTex config: {e}"))?;
    // RaTeX is the default after the representative native/PTY compatibility gate.
    let mut renderer = env
        .map(str::parse)
        .transpose()?
        .or(conf.renderer)
        .unwrap_or(Renderer::Ratex);
    while let Some(arg) = args.first() {
        if arg == "--" {
            args.remove(0);
            break;
        }
        if arg == "--renderer" {
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
        &file,
    )?)
}
#[cfg(test)]
mod tests {
    use super::*;
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
            "renderer='mathjax'",
        )
        .unwrap();
        assert_eq!(options.renderer, Renderer::Ratex);
        assert_eq!(options.command, ["codex", "--renderer=not-ours"]);
        assert_eq!(
            resolve(vec![], Some("mathjax"), "renderer='ratex'")
                .unwrap()
                .renderer,
            Renderer::Mathjax
        );
        assert_eq!(
            resolve(vec![], None, "renderer='ratex'").unwrap().renderer,
            Renderer::Ratex
        );
    }
    #[test]
    fn native_default() {
        let options = resolve(vec![], None, "").unwrap();
        assert_eq!(options.renderer, Renderer::Ratex);
        assert_eq!(options.command, ["codex", "-c", "tui.rendering.math=false"]);
    }
    #[test]
    fn reject_typos() {
        assert!(resolve(vec!["--renderer".into()], None, "").is_err());
        assert!(resolve(vec![], None, "renderer='typo'").is_err());
        assert!(resolve(vec![], None, "rendrer='ratex'").is_err());
    }
}
