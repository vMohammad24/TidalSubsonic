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

        rustToolchain = pkgs.rust-bin.stable."1.97.0".default.override {
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
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Security
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        app = craneLib.buildPackage (commonArgs
          // {
            inherit cargoArtifacts;
            pname = "tss";
            cargoTestExtraArgs = "-- --skip db::tests --skip util::crypto";
          });
      in {
        packages = {
          default = app;

          container = pkgs.dockerTools.buildLayeredImage {
            name = "tss";
            tag = "latest";
            contents = [
              app
              pkgs.cacert
              pkgs.tzdata
              pkgs.busybox
              pkgs.dockerTools.fakeNss
            ];
            config = {
              Cmd = ["${app}/bin/tss"];
              Env = [
                "RUST_LOG=info"
                "HOST=0.0.0.0"
                "PORT=3000"
              ];
              User = "nobody:nobody";
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
        with lib; let
          cfg = config.services.tss;
        in {
          options.services.tss = {
            enable = mkEnableOption "Tidal SubSonic API layer";

            package = mkOption {
              type = types.package;
              default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
              defaultText =
                lib.literalExpression "self.packages.${pkgs.stdenv.hostPlatform.system}.default";
              description = "The TSS package to run.";
            };

            host = mkOption {
              type = types.str;
              default = "127.0.0.1";
              description = "The address TSS should bind to.";
            };

            port = mkOption {
              type = types.port;
              default = 3000;
              description = "The port the TSS service should listen on.";
            };

            logLevel = mkOption {
              type = types.enum ["trace" "debug" "info" "warn" "error"];
              default = "info";
              description = "RUST_LOG level for the service.";
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

          config = mkIf cfg.enable {
            systemd.services.tss = {
              description = "Tidal SubSonic service";
              wantedBy = ["multi-user.target"];
              wants = ["network-online.target"];
              after = ["network-online.target"];

              serviceConfig = {
                Type = "simple";
                ExecStart = "${cfg.package}/bin/tss";
                Restart = "always";
                RestartSec = "5s";

                DynamicUser = true;
                RuntimeDirectory = "tss";
                StateDirectory = "tss";

                Environment = [
                  "HOST=${cfg.host}"
                  "PORT=${toString cfg.port}"
                  "RUST_LOG=${cfg.logLevel}"
                  "HOME=/run/tss"
                ];

                EnvironmentFile = mkIf (cfg.envFile != null) cfg.envFile;
                NoNewPrivileges = true;
                PrivateTmp = true;
                PrivateDevices = true;
                ProtectSystem = "strict";
                ProtectHome = true;
                ProtectKernelTunables = true;
                ProtectKernelModules = true;
                ProtectKernelLogs = true;
                ProtectClock = true;
                ProtectControlGroups = true;
                RestrictRealtime = true;
                RestrictNamespaces = true;
                RestrictAddressFamilies = ["AF_INET" "AF_INET6" "AF_UNIX"];
                LockPersonality = true;
                MemoryDenyWriteExecute = true;
                CapabilityBoundingSet = [];
                SystemCallArchitectures = "native";
                UMask = "0077";
                RemoveIPC = true;
              };
            };
          };
        };
    };
}
