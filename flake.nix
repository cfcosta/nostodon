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

        database = {
          host = "127.0.0.1";
          port = "5432";
          database = "nostodon";
          user = "nostodon";
          password = "nostodon";
        };
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

          NOSTODON_DATABASE_URL =
            "postgres://${database.user}:${database.password}@${database.host}:${database.port}/${database.database}";

          HOST = database.host;
          PORT = database.port;
          DATABASE = database.database;
          USER = database.user;
          PASSWORD = database.password;
        };
      });
}
