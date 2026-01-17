let
  pkgs = import <nixpkgs> {
    overlays = [
      (import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
    ];
  };
  rustToolchain = pkgs.rust-bin.stable.latest.default.override {
    extensions = [ "rust-src" ];
    targets = [
      "wasm32-wasip1"
      "wasm32-unknown-unknown"
    ];
  };
  wasm-bindgen-cli_0_2_106 = pkgs.callPackage ./flakes/wasm-bindgen-cli.nix { };

  # Chrome with web security disabled to allow atomics / bulk-memory operations
  # on sites without cross-origin headers. `dx serve` doesn't set these headers,
  # so we need to disable the security features for local development.
  #
  # Also creates a seperate user profile and sets a custom window name to avoid
  # accidental usage. This browser profile should never be used for normal browsing.
  #
  # https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CORS
  unsafeChromium = pkgs.writeShellScriptBin "chrome-unsafe" ''
    PROFILE_DIR="$PWD/.chrome-unsafe-profile"
    mkdir -p "$PROFILE_DIR"

    exec ${pkgs.chromium}/bin/chromium \
      --user-data-dir="$PROFILE_DIR" \
      --no-first-run \
      --window-name="UNSAFE DEV BROWSER" \
      --disable-web-security \
      --disable-site-isolation-trials \
      --disable-features=IsolateOrigins,site-per-process \
      --enable-features=SharedArrayBuffer \
      --blink-settings=allowSharedArrayBuffer=true \
      http://localhost:8080
  '';

in
pkgs.mkShell {
  packages = with pkgs; [
    rustToolchain
    rust-analyzer
    cargo-sort
    cargo-machete
    samply
    dioxus-cli
    concurrently

    unsafeChromium
    tailwindcss_4
    watchman

    wabt
    wasm-tools

    wrangler

    # For dioxus-cli
    lld
    pkg-config
    gtk3
    cairo
    glib
    webkitgtk_4_1
    libsoup_3
    xdotool
    openssl
    binaryen
    wasm-bindgen-cli_0_2_106
  ];

  shellHook = ''
    echo "Run 'dev' to start the development server"
  '';
}
