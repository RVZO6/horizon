#!/usr/bin/env bash
# Builds the browser version into web/dist. See web/README.md.
#
#   web/build.sh                 the game (cargo profile `fast`)
#   web/build.sh --release       the game, optimized and with LTO (cargo profile `release`)
#   web/build.sh --probe         the platform-layer probe instead (web/www/probe.html)
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(dirname "$here")
toolchain=${HORIZON_WEB_TOOLCHAIN:-nightly}
target_dir=${CARGO_TARGET_DIR:-$root/target}
dist=$here/dist
profile=fast
what=game
for arg in "$@"; do
    case $arg in
        --release) profile=release ;;
        --probe) what=probe ;;
        *) echo "unknown argument: $arg" >&2; exit 2 ;;
    esac
done

# 1. The standard library, patched for the browser (web/std/std.patch). cargo builds it from
#    this copy: __CARGO_TESTS_ONLY_SRC_ROOT replaces the toolchain's rust-src.
src=$(rustc "+$toolchain" --print sysroot)/lib/rustlib/src/rust/library
std=$target_dir/web-std
stamp="$(rustc "+$toolchain" -vV | grep commit-hash) $(shasum "$here/std/std.patch" | cut -d' ' -f1)"
if [[ "$(cat "$std/stamp" 2>/dev/null)" != "$stamp" ]]; then
    echo "Patching the standard library ($toolchain)"
    rm -rf "$std/library" "$std/library.orig"
    mkdir -p "$std"
    cp -R "$src" "$std/library.orig"
    cp -R "$src" "$std/library"
    (cd "$std/library" && patch -p1 --quiet --no-backup-if-mismatch < "$here/std/std.patch")
    echo "$stamp" > "$std/stamp"
fi
export __CARGO_TESTS_ONLY_SRC_ROOT=$std/library

# 2. The program. Threads need shared memory (atomics); the main thread's stack is 8 MB, as on
#    macOS, and wasm32's whole address space (4 GB) may be used.
link=(--shared-memory --import-memory --max-memory=4294967296 -zstack-size=8388608
    --export=__wasm_init_tls --export=__tls_size --export=__tls_align --export=__tls_base)
export RUSTFLAGS="-C target-feature=+atomics,+bulk-memory,+mutable-globals --cfg getrandom_backend=\"wasm_js\" ${link[*]/#/-C link-arg=} ${RUSTFLAGS:-}"
build=(cargo "+$toolchain" build -Z build-std=std,panic_abort --target wasm32-unknown-unknown --profile "$profile")
if [[ $what == probe ]]; then
    "${build[@]}" -p horizon-web --example web_probe
    wasm=$target_dir/wasm32-unknown-unknown/$profile/examples/web_probe.wasm
else
    "${build[@]}" -p horizon-client --bin horizon-client --no-default-features --features audio
    wasm=$target_dir/wasm32-unknown-unknown/$profile/horizon-client.wasm
fi

# 3. JavaScript bindings and the page.
rm -rf "$dist"
mkdir -p "$dist"
wasm-bindgen --target web --out-dir "$dist" --out-name horizon "$wasm"
cp "$here"/www/* "$dist/"
echo "Built $dist ($(du -h "$dist/horizon_bg.wasm" | cut -f1) wasm)"

# 4. The resource bundles, when resources are prepared (docs/native-client.md).
if [[ $what == game && -d $root/.local/horizon-client/26.3 ]]; then
    python3 "$here/bundle.py" --resources "$root/.local/horizon-client/26.3" --out "$dist/fs"
    # The loading bar's totals: the bytes the page receives once decompressed.
    printf '{"wasm": %d, "startup": %d}\n' "$(wc -c < "$dist/horizon_bg.wasm")" \
        "$(wc -c < "$dist/fs/startup.bundle")" > "$dist/sizes.json"
fi
