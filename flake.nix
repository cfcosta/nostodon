{
  description = "A daemon for two way syncing between Mastodon and Nostr";

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
            sqlx-cli
            pgcli
          ];

          DATABASE_URL = "postgres://nostodon:nostodon@localhost:5432/nostodon";
          PGHOST = "127.0.0.1";
          PGPORT = "5432";
          PGDATABASE = "nostodon";
          PGUSER = "nostodon";
          PGPASSWORD = "nostodon";
        };
      });
}
