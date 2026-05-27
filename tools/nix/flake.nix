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

              packages = with pkgs; [
                just
                maturin
                uv
                pkg-config
                openssl
                openssl.dev
              ];

              languages.rust = {
                enable = true;
                channel = "stable";
                components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" ];
              };

              languages.python = {
                enable = true;
                package = pkgs.python312;
              };

              # OpenSSL env vars for crates that use the openssl-sys crate
              # (noodles-htsget pulls in reqwest which may link against it)
              env = {
                OPENSSL_DIR = "${pkgs.openssl.dev}";
                OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
              };
            }];
          };
        }
      );
    };
}
