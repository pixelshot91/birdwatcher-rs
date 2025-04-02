# To build:
#   nix build
# To check the flake:
#   nix flake check
# To get the output artifact of a test:
#   nix derivation show .#checks.x86_64-linux.integration_test | jq --raw-output  .[].outputs.out.path
# To run interactivly:
#   nix run '.#checks.x86_64-linux.integration_test.driverInteractive'
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

        # Common arguments can be set here to avoid repeating them later
        # Note: changes here will rebuild all dependency crates
        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
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
