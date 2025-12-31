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
  devServer = pkgs.writeShellScriptBin "dev" ''
    trap "kill 0" EXIT

    # Start Chrome-unsafe in background (keep logs visible for debugging)
    chrome-unsafe &

    # Start Dioxus in foreground
    dx serve --port 8080 --platform web
  '';
  releaseServer = pkgs.writeShellScriptBin "release" ''
    trap "kill 0" EXIT

    # Start Chrome-unsafe in background (keep logs visible for debugging)
    chrome-unsafe &

    # Start Dioxus in foreground
    dx serve --port 8080 --platform web --release
  '';
  # Unsafe Chrome for testing COOP/COEP locally.
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

    devServer
    releaseServer
    unsafeChromium

    wabt
    wasm-tools

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
    echo "Run 'dev' to start the development server with Caddy proxy."
  '';
}
