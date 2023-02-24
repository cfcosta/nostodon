{
  description = "A daemon for two way syncing between mastodon and nostr";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";

    cargo2nix = {
      url = "github:cfcosta/cargo2nix";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        rust-overlay.follows = "rust-overlay";
      };
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, cargo2nix }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        customPackages = (_: _: {
          inherit nostodon;

          cargo2nix = cargo2nix.packages.${system}.cargo2nix;
        });
        overlays = [ cargo2nix.overlays.default customPackages ];
        pkgs = import nixpkgs { inherit system overlays; };

        database = rec {
          host = "127.0.0.1";
          port = "5432";
          database = "nostodon";
          user = "nostodon";
          password = "nostodon";
          full_url =
            "postgres://${user}:${password}@${host}:${port}/${database}";
        };

        cargoBuild = pkgs:
          pkgs.rustBuilder.makePackageSet {
            rustVersion = "1.67.1";
            packageFun = import ./Cargo.nix;
          };

        nostodon = ((cargoBuild pkgs).workspace.nostodon { }).bin;
        ops.core-server = nixpkgs.lib.nixosSystem {
          inherit system pkgs;

          modules = [
            "${nixpkgs}/nixos/modules/virtualisation/linode-image.nix"
            ./ops/core-server.nix
          ];
        };
      in {
        inherit ops;
        defaultPackage = ops.core-server.config.system.build.toplevel;

        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            cargo2nix.packages.${system}.cargo2nix
            cargo-watch
            openssl
            pgcli
            pkg-config
            postgresql
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" "clippy" "rustfmt" "rust-analyzer" ];
            })
            sqlx-cli
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
