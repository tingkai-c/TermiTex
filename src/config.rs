use serde::Deserialize;
use std::{path::PathBuf, str::FromStr};
// Claude's Markdown renderer consumes a single backslash before punctuation.
// Request two in the response so the terminal retains our \( / \[ delimiters.
const CLAUDE_MATH_PROMPT: &str = r"Write LaTeX math outside code formatting; double delimiter backslashes: \\(...\\) inline, \\[...\\] display. For display math, put delimiters on separate lines with blank lines around the body.";
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Graphics {
    #[default]
    Auto,
    Kitty,
    Off,
}
impl FromStr for Graphics {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "auto" => Ok(Self::Auto),
            "kitty" => Ok(Self::Kitty),
            "off" => Ok(Self::Off),
            _ => Err(format!(
                "unknown graphics mode {s:?}; choose auto, kitty or off"
            )),
        }
    }
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    graphics: Option<Graphics>,
    renderer: Option<Renderer>,
    layout: Option<Layout>,
}
pub struct Options {
    pub graphics: Graphics,
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
    let mut graphics = conf.graphics.unwrap_or_default();
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
    let mut literal = false;
    while let Some(arg) = args.first() {
        if arg == "--" {
            literal = true;
            args.remove(0);
            break;
        }
        if arg == "--graphics" {
            args.remove(0);
            if args.is_empty() {
                return Err("--graphics requires auto, kitty or off".into());
            }
            graphics = args.remove(0).parse()?;
        } else if let Some(name) = arg.strip_prefix("--graphics=") {
            graphics = name.parse()?;
            args.remove(0);
        } else if arg == "--layout" {
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
        } else if arg.starts_with('-') {
            return Err(format!(
                "unknown TermiTex option {arg:?}; put app arguments after the command"
            ));
        } else {
            break;
        }
    }
    if !args.is_empty() && !literal {
        match std::path::Path::new(&args[0])
            .file_name()
            .and_then(|n| n.to_str())
        {
            Some("codex") => {
                args.splice(1..1, ["-c".into(), "tui.rendering.math=false".into()]);
            }
            Some("claude") => {
                // Respect explicit append instructions, including file-based
                // prompts. Passing duplicate flags can replace the user's text
                // or be incompatible with older Claude Code versions.
                let custom_append = args[1..].iter().take_while(|a| *a != "--").any(|a| {
                    a == "--append-system-prompt"
                        || a.starts_with("--append-system-prompt=")
                        || a == "--append-system-prompt-file"
                        || a.starts_with("--append-system-prompt-file=")
                });
                if !custom_append {
                    args.splice(
                        1..1,
                        ["--append-system-prompt".into(), CLAUDE_MATH_PROMPT.into()],
                    );
                }
            }
            _ => {}
        }
    }
    Ok(Options {
        graphics,
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
    fn intuitive_commands_and_literal_passthrough() {
        let resolve_args = |args: &[&str]| {
            resolve(args.iter().map(|s| s.to_string()).collect(), None, None, "")
                .unwrap()
                .command
        };
        assert_eq!(
            resolve_args(&["claude", "--model", "sonnet", "--help"]),
            [
                "claude",
                "--append-system-prompt",
                CLAUDE_MATH_PROMPT,
                "--model",
                "sonnet",
                "--help"
            ]
        );
        assert_eq!(
            resolve_args(&["codex", "resume"]),
            ["codex", "-c", "tui.rendering.math=false", "resume"]
        );
        assert_eq!(
            resolve_args(&["--", "codex", "resume"]),
            ["codex", "resume"]
        );
        assert_eq!(
            resolve_args(&["python", "-c", "print(1)"]),
            ["python", "-c", "print(1)"]
        );
        assert!(resolve(vec!["--typo".into()], None, None, "").is_err());
    }
    #[test]
    fn claude_math_prompt_preserves_flags_and_explicit_prompts() {
        // Single backslashes disappear in Claude's Markdown output. Keep the
        // response examples escaped so its terminal output retains delimiters.
        assert!(CLAUDE_MATH_PROMPT.contains(r"\\(...\\)"));
        assert!(CLAUDE_MATH_PROMPT.contains(r"\\[...\\]"));
        let resolve_args = |args: &[&str]| {
            resolve(args.iter().map(|s| s.to_string()).collect(), None, None, "")
                .unwrap()
                .command
        };
        assert_eq!(
            resolve_args(&[
                "/usr/local/bin/claude",
                "--model",
                "sonnet",
                "--continue",
                "Explain x squared"
            ]),
            [
                "/usr/local/bin/claude",
                "--append-system-prompt",
                CLAUDE_MATH_PROMPT,
                "--model",
                "sonnet",
                "--continue",
                "Explain x squared"
            ]
        );
        for args in [
            vec!["--", "claude", "--model", "sonnet"],
            vec![
                "claude",
                "--append-system-prompt",
                "My formatting",
                "--continue",
            ],
            vec!["claude", "--append-system-prompt=My formatting"],
            vec!["claude", "--append-system-prompt-file", "my rules.txt"],
            vec!["claude", "--append-system-prompt-file=my rules.txt"],
        ] {
            let expected = if args[0] == "--" {
                &args[1..]
            } else {
                &args[..]
            };
            assert_eq!(resolve_args(&args), expected);
        }
        assert_eq!(
            resolve_args(&["claude", "--", "--append-system-prompt"]),
            [
                "claude",
                "--append-system-prompt",
                CLAUDE_MATH_PROMPT,
                "--",
                "--append-system-prompt"
            ]
        );
    }
    #[test]
    fn graphics_config_override_and_child_boundary() {
        assert_eq!(
            resolve(vec![], None, None, "").unwrap().graphics,
            Graphics::Auto
        );
        assert_eq!(
            resolve(vec![], None, None, "graphics='off'")
                .unwrap()
                .graphics,
            Graphics::Off
        );
        let opts = resolve(
            vec![
                "--graphics=kitty".into(),
                "--".into(),
                "app".into(),
                "--graphics=off".into(),
            ],
            None,
            None,
            "graphics='off'",
        )
        .unwrap();
        assert_eq!(opts.graphics, Graphics::Kitty);
        assert_eq!(opts.command, ["app", "--graphics=off"]);
        assert!(resolve(vec!["--graphics".into()], None, None, "").is_err());
        assert!(resolve(vec![], None, None, "graphics='typo'").is_err());
    }
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
        assert!(options.command.is_empty());
    }
    #[test]
    fn reject_typos() {
        assert!(resolve(vec!["--renderer".into()], None, None, "").is_err());
        assert!(resolve(vec![], None, None, "renderer='typo'").is_err());
        assert!(resolve(vec![], None, None, "rendrer='ratex'").is_err());
    }
}
