{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      rust-overlay,
      advisory-db,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        inherit (pkgs) lib;
        src = craneLib.cleanCargoSource ./.;
        craneLib = (crane.mkLib pkgs).overrideToolchain (p: p.rust-bin.stable.latest.default);

        commonArgs = {
          inherit src;
          strictDeps = true;
          buildInputs =
            [ ]
            ++ lib.optionals pkgs.stdenv.isDarwin [
              pkgs.libiconv
            ];
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        individualCrateArgs = commonArgs // {
          inherit cargoArtifacts;
          inherit (craneLib.crateNameFromCargoToml { inherit src; }) version;
          doCheck = false;
        };

        fileSetForCrate =
          crate: deps:
          lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions (
              [
                ./Cargo.toml
                ./Cargo.lock
                (craneLib.fileset.commonCargoSources ./crates/mjd-workspace-hack)
                (craneLib.fileset.commonCargoSources crate)
              ]
              ++ map (dep: craneLib.fileset.commonCargoSources dep) deps
            );
          };
        mjl = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "mjl";
            cargoExtraArgs = "-p mjl";
            src = fileSetForCrate ./crates/mjl [];
          }
        );
        mjp = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "mjp";
            cargoExtraArgs = "-p mjp";
            src = fileSetForCrate ./crates/mjp [./crates/mjl];
          }
        );
      in
      {
        checks = {
          inherit mjl mjp;

          workspace-clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          workspace-doc = craneLib.cargoDoc (
            commonArgs
            // {
              inherit cargoArtifacts;
              env.RUSTDOCFLAGS = "--deny warnings";
            }
          );

          workspace-fmt = craneLib.cargoFmt {
            inherit src;
          };

          workspace-toml-fmt = craneLib.taploFmt {
            src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
          };

          workspace-audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };

          workspace-deny = craneLib.cargoDeny {
            inherit src;
          };

          workspace-nextest = craneLib.cargoNextest (
            commonArgs
            // {
              inherit cargoArtifacts;
              partitions = 1;
              partitionType = "count";
              cargoNextestPartitionsExtraArgs = "--no-tests=pass";
            }
          );

          workspace-hakari = craneLib.mkCargoDerivation {
            inherit src;
            pname = "mjd-workspace-hakari";
            cargoArtifacts = null;
            doInstallCargoArtifacts = false;

            buildPhaseCargoCommand = ''
              cargo hakari generate --diff # workspace-hack Cargo.toml is up-to-date
              cargo hakari manage-deps --dry-run # all workspace crates depend on workspace-hack
              cargo hakari verify
            '';

            nativeBuildInputs = [
              pkgs.cargo-hakari
            ];
          };
        };
        packages.default = craneLib.buildPackage {
          inherit src;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          packages = [
            pkgs.cargo-hakari
          ];
        };
      }
    );
}
