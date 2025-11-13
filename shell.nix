{
  pkgs ? import <nixpkgs> { },
  unstable ? import <nixpkgs-unstable> { },
}:

pkgs.mkShell {
  packages = with pkgs; [
    cargo
    rustc
    rustfmt
    dioxus-cli
    lld # Required for dioxus
    pkg-config # Required for dioxus
    gtk3 # Required for dioxus
    cairo # Required for dioxus
    glib # Required for dioxus
    webkitgtk_4_1 # Required for dioxus
    libsoup_3 # Required for dioxus
    xdotool # Required for dioxus
    unstable.wasm-bindgen-cli_0_2_104 # Required for dioxus
  ];
}
