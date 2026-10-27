//! Horizon: native Rust client for Minecraft Java Edition 26.3 gameplay.
// The browser's entry point is a wasm-bindgen export, whose generated glue is unsafe.
#![cfg_attr(not(target_arch = "wasm32"), forbid(unsafe_code))]
mod game;

use horizon_client::{crash, logging};

/// The mod API level this game serves, kept in the program for the launcher
/// to read without running it (`horizon_mods::api_level::MARK`). `main`
/// reads it, so the linker keeps it.
static MOD_API_LEVEL: [u8; 27] = horizon_mods::api_level::MARK;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    start();
}

/// wasm-bindgen runs `main` on every thread it starts: the page calls `horizon_run` instead.
#[cfg(target_arch = "wasm32")]
fn main() {}

/// The browser's entry point (web/www/main.js): opens the GPU device first, as WebGPU only
/// answers once the page's thread returns to the browser, then starts the game as `main` does.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn horizon_run() -> Result<(), wasm_bindgen::JsError> {
    // Linked for the hooks web/std calls.
    use horizon_web as _;
    horizon_client::gfx::open_web_device()
        .await
        .map_err(|e| wasm_bindgen::JsError::new(&e.to_string()))?;
    start();
    Ok(())
}

fn start() {
    std::hint::black_box(&MOD_API_LEVEL);
    horizon_client::telemetry::mark_start();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let game_args = args.iter().take_while(|a| *a != "--dedicated");
    let help = game_args.clone().any(|a| a == "--help" || a == "-h");
    // The dedicated server logs in its own folder, as vanilla's does; the game in its home.
    let (home, side, thread) = if args.iter().any(|a| a == "--dedicated") {
        (std::path::PathBuf::from("."), "server", "Server thread")
    } else {
        (logging::game_home(), "client", "Render thread")
    };
    if !help {
        logging::init(&home, thread);
        // The log's first line names the build, so a bug report that carries the log says which one it was.
        log::info!("Horizon {}", crash::version());
        crash::install(home.clone(), side);
    }
    if let Err(error) = game::run() {
        log::error!("Horizon: {error}");
        if !help {
            crash::report_error(&home, side, &*error);
        }
        // Started from Explorer, the console window closes with the game:
        // keep the message on screen until it is read.
        #[cfg(windows)]
        if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            eprintln!("Press Enter to close.");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
        std::process::exit(1);
    }
    logging::flush();
}
