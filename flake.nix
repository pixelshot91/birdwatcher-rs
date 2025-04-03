# To build:
#   nix build
# To check the flake:
#   nix flake check
# To get the output artifact of a test:
#   nix derivation show .#checks.x86_64-linux.integration_test | jq --raw-output  .[].outputs.out.path
# To run interactivly:
#   nix run '.#checks.x86_64-linux.integration_test.driverInteractive'

# The NixOs testing part was inpired from
#   https://blakesmith.me/2024/03/02/running-nixos-tests-with-flakes.html
{
  description = "Build a cargo project without extra checks";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      rust-overlay,
      ...
    }:
    {
      nixosModules = {
        birdwatcher-rs-NixosModule = import ./birdwatcher-rs-module.nix;
      };
    }
    // flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [
          (import rust-overlay)
          # self.packages.${system}.default
        ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        # pkgs = nixpkgs.legacyPackages.${system}.extend overlays;

        craneLib = (crane.mkLib pkgs).overrideToolchain (
          p: p.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml
        );

        unfilteredRoot = ./.; # The original, unfiltered source
        src = pkgs.lib.fileset.toSource {
          root = unfilteredRoot;
          fileset = pkgs.lib.fileset.unions [
            # Default files from crane (Rust and cargo files)
            (craneLib.fileset.commonCargoSources unfilteredRoot)
            # Include the `example/` folder because it is checked for correctness in config.rs test
            (pkgs.lib.fileset.maybeMissing ./example)
          ];
        };

        # Common arguments can be set here to avoid repeating them later
        # Note: changes here will rebuild all dependency crates
        commonArgs = {
          inherit src;

          # Ensure buildInputs and nativeBuildInputs distinction are well respected
          # Useful for cross-compilation. Should always be true ?
          # See: https://github.com/NixOS/nixpkgs/pull/354949/files
          strictDeps = true;

          buildInputs = [
            # Add additional build inputs here
          ];
        };

        birdwatcher-rs = craneLib.buildPackage (
          commonArgs
          // {
            cargoArtifacts = craneLib.buildDepsOnly commonArgs;

            # Additional environment variables or build phases/hooks can be set
            # here *without* rebuilding all dependency crates
            # MY_CUSTOM_VAR = "some value";
          }
        );
      in
      {
        checks =
          let
            overlay = final: prev: {
              birdwatcher-rs = self.packages.${system}.default; # birdwatcher-rs;
            };
            mypkgs = nixpkgs.legacyPackages.${system}.extend overlay;
          in
          {
            # inherit birdwatcher-rs;

            integration_test = pkgs.callPackage ./integration_test/single_service.nix {
              inherit self;
              pkgs = mypkgs;
            };
          };

        packages.default = birdwatcher-rs;

        devShells.default = craneLib.devShell {
          # Inherit inputs from checks.
          checks = self.checks.${system};

          # Additional dev-shell environment variables can be set directly
          # MY_CUSTOM_DEVELOPMENT_VAR = "something else";

          # Extra inputs can be added here; cargo and rustc are provided by default.
          packages = [
            # pkgs.ripgrep
          ];
        };
      }
    );
}
