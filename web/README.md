# Horizon in the browser

The game, built for `wasm32-unknown-unknown` with threads, drawn with WebGPU.

```sh
web/build.sh             # the game (cargo profile `fast`) into web/dist; --release for LTO
python3 web/serve.py     # http://127.0.0.1:8026/, `?arg=--world&arg=Test` opens a world
```

It needs a nightly toolchain with `rust-src` (`rustup toolchain install nightly --component
rust-src --target wasm32-unknown-unknown`), `wasm-bindgen-cli` of the version in `Cargo.lock`,
`brotli`, and prepared resources in `.local/horizon-client/26.3` (docs/native-client.md).
A browser needs WebGPU and shared memory: a secure context (HTTPS or localhost) and the
cross-origin isolation headers web/serve.py sends.

## How it works

- **`web/std/std.patch`** gives Rust's standard library a browser backend, so the game's threads,
  files, clocks and environment work unchanged: threads are Web Workers on the shared memory, the
  file system lives in memory, time and the console come from the page. The patch applies to the
  nightly's `rust-src`; `build.sh` builds std from that copy (`-Z build-std`). The browser's main
  thread may not block, so there std spins where it would wait.
- **`crates/horizon-web`** defines the functions that patch calls (`__horizon_web_*`), the entry
  points of the page, and the allocator (small blocks cached per thread, in front of dlmalloc's
  single lock).
- **Threads.** A worker only starts once the thread that made it returns to its event loop, which
  the game's threads never do. `www/spawner.js`, a worker that never blocks, keeps a pool of
  workers ready; each waits on shared memory for the next `std::thread::spawn`.
- **Files.** `web/bundle.py` packs the prepared resources: `startup.bundle` (what the game reads
  before its first world) streams into memory with the wasm module, `later.bundle` (structure
  templates) once the game runs; any other file (sounds, other languages) is fetched when the game
  first reads it.
- **GPU.** WebGPU answers through promises, which settle only when the page's thread returns to
  the browser, so `horizon_run` opens the device before the game starts and error scopes count as
  clean (`gfx::scope_error`).
- **Loading.** `www/overlay.js` is vanilla's Mojang Studios screen (26.3's `LoadingOverlay`) over
  the downloads and the game's start, then its fade into the title screen.

## Not yet

Sound, saving across visits (the file system starts empty on each load), mobile controls,
`wasm-opt`.
