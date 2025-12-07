let
  pkgs = import <nixpkgs> {
    overlays = [
      (import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
    ];
  };
  unstable = import <nixpkgs-unstable> { };
  wasm-bindgen-cli_0_2_106 = pkgs.callPackage ./flakes/wasm-bindgen-cli.nix { };
in
pkgs.mkShell {
  packages = with pkgs; [
    # Rust toolchain with WASI target
    (rust-bin.stable.latest.minimal.override {
      extensions = [
        "rust-src"
        "cargo"
        "rustc"
        "clippy"
      ];
      targets = [
        "wasm32-wasip1"
        "wasm32-unknown-unknown"
      ];
    })

    rust-bin.stable.latest.cargo
    rust-bin.stable.latest.rust-analyzer
    rust-bin.stable.latest.clippy
    rust-bin.nightly.latest.rustfmt
    cargo-sort
    cargo-machete
    samply
    dioxus-cli

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
}
