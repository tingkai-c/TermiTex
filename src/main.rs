mod app;
mod config;
mod detect;
mod engine;
mod frame;
mod graphics;
mod layout;
mod native;
mod renderer;
mod session;
mod table;
mod terminal;

fn main() {
    match run() {
        Ok(status) => std::process::exit(status),
        Err(e) => {
            eprintln!("termitex: {e}");
            std::process::exit(1);
        }
    }
}
fn usage() {
    println!(
        "Usage: termitex [options] <app> [args...]

Examples:
  termitex codex
  termitex claude
  termitex --layout compatibility python

Run termitex --help for all options."
    );
}
fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
        return Ok(0);
    }
    if args.first().is_some_and(|s| s == "--internal-ratex-worker") {
        native::worker()?;
        return Ok(0);
    }
    if args.first().is_some_and(|s| s == "--version" || s == "-V") {
        println!("termitex {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }
    if args.first().is_some_and(|s| s == "--help" || s == "-h") {
        println!("TermiTex: asynchronous math rendering for Kitty-graphics terminals
termitex [options] <app> [app arguments...]
Examples: termitex codex | termitex claude | termitex python
termitex [--renderer ratex|mathjax] [--layout compact|compatibility] [--graphics auto|kitty|off] [--] command [args...]
termitex doctor                 Report terminal capabilities without launching an application.
Options belong before the app; arguments after it pass through unchanged.
Direct codex launches disable its built-in math rendering. Use -- for literal command passthrough.
Default: native RaTeX, compact layout, auto-detected graphics. Without an app, shows usage.
Unsupported terminals keep original text. --graphics kitty overrides detection; off disables graphics.
Config: ~/.config/termitex/config.toml (or XDG_CONFIG_HOME).
TERMITEX_RENDERER / TERMITEX_LAYOUT override config. CLI options take precedence.
TERMITEX_FG / TERMITEX_BG override colors. TERMITEX_STATS=/path.json saves timing counters.
WezTerm: enable_kitty_graphics = true. See docs/terminals.md for terminal testing and setup.");
        return Ok(0);
    }
    let doctor = args
        .first()
        .is_some_and(|s| s == "doctor" || s == "--doctor");
    if doctor {
        args.remove(0);
    }
    let options = config::load(args)?;
    if !doctor && options.command.is_empty() {
        usage();
        return Ok(0);
    }
    session::run(options, doctor)
}
