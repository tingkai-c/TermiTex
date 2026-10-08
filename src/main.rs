mod config;
mod detect;
mod engine;
mod frame;
mod graphics;
mod layout;
mod native;
mod renderer;
mod session;
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
fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
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
termitex [--renderer ratex|mathjax] [--layout compact|compatibility] [--graphics auto|kitty|off] [--] command [args...]
termitex doctor                 Report terminal capabilities without launching an application.
Default: native RaTeX, compact layout, auto-detected graphics; launches stock Codex.
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
    session::run(config::load(args)?, doctor)
}
