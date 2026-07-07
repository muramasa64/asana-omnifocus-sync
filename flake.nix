{
  description = "Sync Asana tasks assigned to me into OmniFocus";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" ];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # cargo ソースに加え、include_str! で埋め込む scripts/*.js も残す。
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            (pkgs.lib.hasSuffix ".js" path) || (craneLib.filterCargoSources path type);
        };

        commonArgs = {
          inherit src;
          strictDeps = true;
          # 最近の darwin stdenv は Apple SDK（Security 等）を自動提供するため、
          # native-tls / security-framework のリンクに追加の buildInputs は不要。
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        bin = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        }) // {
          meta = {
            description = "Sync Asana tasks assigned to me into OmniFocus";
            mainProgram = "asana-omnifocus-sync";
            # OmniFocus 連携に JXA（osascript）を使うため macOS 専用。
            platforms = pkgs.lib.platforms.darwin;
          };
        };
      in
      {
        packages.default = bin;

        checks = {
          inherit bin;
          clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });
          test = craneLib.cargoTest (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        devShells.default = craneLib.devShell {
          packages = [ rustToolchain ];
        };
      });
}
