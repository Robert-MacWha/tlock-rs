let
  pkgs = import <nixpkgs> {
    overlays = [
      (import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
    ];
  };
  unstable = import <nixpkgs-unstable> { };
in
pkgs.mkShell {
  packages = with pkgs; [
    # Rust toolchain with WASI target
    (rust-bin.stable.latest.default.override {
      extensions = [ "rust-src" ];
      targets = [
        "wasm32-wasip1"
        "wasm32-unknown-unknown"
      ];
    })

    cargo
    rustfmt
    rust-bin.stable.latest.rust-analyzer
    unstable.dioxus-cli

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
    wasm-bindgen-cli_0_2_104
  ];
}
