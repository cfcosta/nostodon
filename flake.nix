{
  description = "An agent to sync mastodon posts to nostr";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs { inherit system overlays; };
      in {
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            cargo-watch
            pkg-config
            openssl
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" "clippy" "rustfmt" "rust-analyzer" ];
            })
          ];
        };
      });
}
