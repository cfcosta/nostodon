{
  description = "A daemon for two way syncing between mastodon and nostr";

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

        database = rec {
          host = "127.0.0.1";
          port = "5432";
          database = "nostodon";
          user = "nostodon";
          password = "nostodon";
          full_url = "postgres://${user}:${password}@${host}:${port}/${database}";
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

          NOSTODON_DATABASE_URL = database.full_url;
          DATABASE_URL = database.full_url;

          PGHOST = database.host;
          PGPORT = database.port;
          PGDATABASE = database.database;
          PGUSER = database.user;
          PGPASSWORD = database.password;
        };
      });
}
