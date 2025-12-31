{
  description = "Backup your GitHub repositories automatically.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";

    flake-utils.url = "github:numtide/flake-utils";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, crane, fenix, flake-utils, advisory-db, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        inherit (pkgs) lib;

        craneLib = crane.mkLib pkgs;

        testDataFilter = path: _type: builtins.match ".*/tests/.*$" path != null;
        sourceFilter = path: type: (testDataFilter path type) || (craneLib.filterCargoSources path type);

        src = lib.cleanSourceWith {
          src = ./.;
          filter = sourceFilter;
          name = "source";
        };

        # Common arguments can be set here to avoid repeating them later
        commonArgs = {
          inherit src;
          strictDeps = true;

          buildInputs = []
          ++ lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];

          # Additional environment variables can be set directly
          # MY_CUSTOM_VAR = "some value";
        };

        craneLibLLvmTools = craneLib.overrideToolchain
          (fenix.packages.${system}.complete.withComponents [
            "cargo"
            "llvm-tools"
            "rustc"
          ]);

        # Build *just* the cargo dependencies, so we can reuse
        # all of that work (e.g. via cachix) when running in CI
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Build the actual crate itself, reusing the dependency
        # artifacts from above.
        github-backup = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;

          # Disable default tests - nextest will run them
          doCheck = false;
        });
      in
      {
        checks = {
          # Build the crate as part of `nix flake check` for convenience
          inherit github-backup;

          # Run clippy (and deny all warnings) on the crate source,
          # again, reusing the dependency artifacts from above.
          #
          # Note that this is done as a separate derivation so that
          # we can block the CI if there are issues here, but not
          # prevent downstream consumers from building our crate by itself.
          github-backup-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

          github-backup-doc = craneLib.cargoDoc (commonArgs // {
            inherit cargoArtifacts;
          });

          # Check formatting
          github-backup-fmt = craneLib.cargoFmt {
            inherit src;
          };

          # Audit dependencies
          github-backup-audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };

          # Audit licenses
          github-backup-deny = craneLib.cargoDeny {
            inherit src;

            cargoDenyChecks = "bans sources";
          };

          # Run tests with cargo-nextest
          # Consider setting `doCheck = false` on `github-backup` if you do not want
          # the tests to run twice
          github-backup-nextest = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";

            cargoNextestExtraArgs = "--no-fail-fast --features pure_tests";
          });
        };

        packages = {
          default = github-backup;
        } // lib.optionalAttrs (!pkgs.stdenv.isDarwin) {
          github-backup-llvm-coverage = craneLibLLvmTools.cargoLlvmCov (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        apps.default = flake-utils.lib.mkApp {
          drv = github-backup;
        };

        devShells.default = craneLib.devShell {
          # Inherit inputs from checks.
          checks = self.checks.${system};

          # Additional dev-shell environment variables can be set directly
          # MY_CUSTOM_DEVELOPMENT_VAR = "something else";

          # Extra inputs can be added here; cargo and rustc are provided by default.
          packages = [
            # pkgs.ripgrep
            pkgs.rustfmt
            pkgs.rust-analyzer
            pkgs.nodejs
          ];
        };
      });
}
