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
              ];

              languages.rust = {
                enable = true;
                channel = "stable";
                components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" ];
              };

              # A uv-managed .venv (nix CPython) that `just develop` builds the
              # extension and its test extras into.
              languages.python = {
                enable = true;
                package = pkgs.python312;
                venv = {
                  enable = true;
                  requirements = ''
                    pytest
                  '';
                };
                uv.enable = true;
              };

              # NixOS: don't let uv fetch a prebuilt interpreter (missing FHS
              # loader); it installs into the active venv instead. Don't set
              # UV_PYTHON — it outranks VIRTUAL_ENV and would target the
              # immutable /nix/store interpreter.
              env.UV_PYTHON_DOWNLOADS = "never";
            }];
          };
        }
      );
    };
}
