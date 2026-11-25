//! Horizon: native Rust client for Minecraft Java Edition 26.3 gameplay.
// The browser's entry point is a wasm-bindgen export, whose generated glue is unsafe.
#![cfg_attr(not(target_arch = "wasm32"), forbid(unsafe_code))]

/// Identifies this executable as the game for launcher imports. `main` reads it so the
/// linker keeps it; it carries neither an API version nor a per-build id.
static GAME_IDENTITY: [u8; horizon_mod_package::game_identity::MARK.len()] =
    horizon_mod_package::game_identity::MARK;

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
    horizon_guest_stack::run_main(|| {
        std::hint::black_box(&GAME_IDENTITY);
        horizon_game::main();
    });
}
