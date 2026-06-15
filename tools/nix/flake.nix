{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    devenv.url = "github:cachix/devenv";
    devenv.inputs.nixpkgs.follows = "nixpkgs";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  nixConfig = {
    extra-trusted-public-keys = "devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw=";
    extra-substituters = "https://devenv.cachix.org";
  };

  outputs = { nixpkgs, devenv, ... } @ inputs:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in {
      devShells = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system}; in {
          default = devenv.lib.mkShell {
            inherit inputs pkgs;
            modules = [{
              # flake.nix lives in tools/nix/ — point devenv at the writable
              # repo root on disk. builtins.getEnv requires --impure.
              devenv.root =
                let pwd = builtins.getEnv "PWD"; in
                if pwd != "" then pwd else builtins.toString ../..;

              # uv is provided by languages.python.uv below, not here.
              packages = with pkgs; [
                just
                maturin
                ruff
                pkg-config
                openssl
                openssl.dev
              ];

              languages.rust = {
                enable = true;
                channel = "stable";
                components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" ];
              };

              # Create and activate a .venv from the nix-provided CPython, with
              # uv for installs. maturin develop installs the extension into
              # this venv (VIRTUAL_ENV); pytest is preinstalled so `just test`
              # works without a manual install step.
              languages.python = {
                enable = true;
                package = pkgs.python312;
                venv = {
                  enable = true;
                  requirements = ''
                    pytest
                    testcontainers
                  '';
                };
                uv.enable = true;
              };

              env = {
                # NixOS: never let uv download a prebuilt interpreter. Those are
                # dynamically linked against an FHS loader (/lib64/ld-linux...)
                # that does not exist on NixOS, so they fail to run. With this
                # off, uv installs into the active venv (VIRTUAL_ENV).
                #
                # Do NOT set UV_PYTHON: it outranks VIRTUAL_ENV in uv's
                # interpreter search, so uv would target the immutable
                # /nix/store interpreter and fail with "externally managed".
                UV_PYTHON_DOWNLOADS = "never";

                # OpenSSL for crates that link openssl-sys (noodles-htsget pulls
                # in reqwest, which may link against it).
                OPENSSL_DIR = "${pkgs.openssl.dev}";
                OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
              };
            }];
          };
        }
      );
    };
}
