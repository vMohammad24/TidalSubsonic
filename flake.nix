{
  description = "Tidal SubSonic - Subsonic-compatible API layer for Tidal";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
    crane,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [(import rust-overlay)];
        };

        rustToolchain = pkgs.rust-bin.stable."1.95.0".default.override {
          extensions = ["rust-src" "rust-analyzer"];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        sqlxAndTemplatesFilter = path: type:
          (builtins.match ".*templates.*" path != null)
          || (builtins.match ".*migrations.*" path != null)
          || (builtins.match ".*\\.sqlx.*" path != null)
          || (craneLib.filterCargoSources path type);

        src = pkgs.lib.cleanSourceWith {
          src = craneLib.path ./.;
          filter = sqlxAndTemplatesFilter;
        };

        commonArgs = {
          inherit src;
          strictDeps = true;

          SQLX_OFFLINE = "true";

          nativeBuildInputs = [pkgs.pkg-config];
          buildInputs =
            [pkgs.openssl]
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.apple_sdk.frameworks.Security
            ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        app = craneLib.buildPackage (commonArgs
          // {
            inherit cargoArtifacts;
            pname = "tss";
          });
      in {
        packages = {
          default = app;

          container = pkgs.dockerTools.buildImage {
            name = "tss";
            tag = "latest";
            copyToRoot = [
              app
              pkgs.cacert
              pkgs.tzdata
              pkgs.curl
            ];
            config = {
              Cmd = ["${app}/bin/tss"];
              Env = ["RUST_LOG=info"];
              ExposedPorts = {"3000/tcp" = {};};
            };
          };
        };

        devShells.default = craneLib.devShell {
          inputsFrom = [app];
          packages = [
            rustToolchain
            pkgs.sqlx-cli
          ];
          shellHook = ''
            export SQLX_OFFLINE=true
          '';
        };
      }
    )
    // {
      nixosModules.default = {
        config,
        lib,
        pkgs,
        ...
      }:
        with lib; {
          options.services.tss = {
            enable = mkEnableOption "Tidal SubSonic API layer";

            port = mkOption {
              type = types.port;
              default = 3000;
              description = "The port the TSS service should listen on.";
            };

            envFile = mkOption {
              type = types.nullOr types.path;
              default = null;
              example = "/run/secrets/tss-env";
              description = ''
                Path to a file containing environment variables (e.g., database credentials).
              '';
            };
          };

          config = mkIf config.services.tss.enable {
            systemd.services.tss = {
              description = "TSS Service";
              wantedBy = ["multi-user.target"];
              after = ["network.target"];
              serviceConfig = {
                ExecStart = "${self.packages.${pkgs.system}.default}/bin/tss";
                Restart = "always";
                DynamicUser = true;

                Environment = [
                  "PORT=${toString config.services.tss.port}"
                ];

                EnvironmentFile = mkIf (config.services.tss.envFile != null) config.services.tss.envFile;
              };
            };
          };
        };
    };
}
