{
  nixConfig.bash-prompt-prefix = ''\[\e[0;31m\](lovely) \e[0m'';

  inputs = {
    # requires nix `>=v2.27`, determinate-nix `v3`, or lix `>=v2.94`
    self.submodules = true;

    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";

    flake-utils.url = "github:numtide/flake-utils";

    gitignore = {
      url = "github:hercules-ci/gitignore.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs:
    inputs.flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [inputs.rust-overlay.overlays.default];
        };
        rust-toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        rust-platform = pkgs.makeRustPlatform {
          cargo = rust-toolchain;
          rustc = rust-toolchain;
        };

        pname = (pkgs.lib.importTOML ./crates/lovely-unix/Cargo.toml).package.name;
        version = (pkgs.lib.importTOML ./crates/lovely-core/Cargo.toml).package.version;
        src = pkgs.lib.cleanSourceWith {
          name = "${pname}-${version}-clean-src";
          src = ./.;
          filter = inputs.gitignore.lib.gitignoreFilterWith {
            basePath = ./.;
            extraRules =
              # gitignore
              ''
                flake.*
                LICENSE.md
                README.md
                .github
              '';
          };
        };
        drv = rust-platform.buildRustPackage {
          inherit src pname version;

          doCheck = false;

          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes."retour-0.4.0-alpha.2" = "sha256-GtLTjErXJIYXQaOFLfMgXb8N+oyHNXGTBD0UeyvbjrA=";
          };
          cargoBuildFlags = ["--package" "lovely-unix"];

          nativeBuildInputs = with pkgs; [cmake];

          env.RUST_BACKTRACE = 1;
        };
      in {
        packages = {
          # `nix build git+https://github.com/ethangreen-dev/lovely-injector && ls result/lib`
          default = inputs.self.packages.${system}.lovely-injector;
          lovely-injector = drv;
        };

        devShells = {
          # `nix develop git+https://github.com/ethangreen-dev/lovely-injector`
          default = inputs.self.devShells.${system}.base;

          base = pkgs.mkShell {
            # grab all build dependencies of all exposed packages
            inputsFrom = pkgs.lib.attrValues inputs.self.packages.${system};
            shellHook = ''echo "with löve from wrd :)"'';
          };

          # `nix develop git+https://github.com/ethangreen-dev/lovely-injector#full`
          full = pkgs.mkShell {
            # inherit the base shell
            inputsFrom = [inputs.self.devShells.${system}.base];
            packages =
              (with pkgs; [luajit love])
              ++ [
                (rust-toolchain.override
                  {extensions = ["rust-src" "rust-analyzer"];})
              ];
          };
        };
      }
    );
}
