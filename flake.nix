{
  description = "Execution Engine - Open-source plugin execution engine";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        hiddenRust = fenix.packages.${system}.stable;

        isDarwin = pkgs.stdenv.isDarwin;
        ldLibraryPath = if isDarwin then "" else "${pkgs.stdenv.cc.cc.lib}/lib:";

        agentBuild = pkgs.writeShellScriptBin "agent-build" ''
          echo "Building Execution Engine..."
          export LIBCLANG_PATH=${pkgs.libclang.lib}/lib
          export LD_LIBRARY_PATH=${ldLibraryPath}${pkgs.openssl.out}/lib:${pkgs.zlib}/lib:''${LD_LIBRARY_PATH:-}
          export OPENSSL_DIR=${pkgs.openssl.dev}
          export OPENSSL_LIB_DIR=${pkgs.openssl.out}/lib
          export OPENSSL_INCLUDE_DIR=${pkgs.openssl.dev}/include
          export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:''${PKG_CONFIG_PATH:-}"
          export PKG_CONFIG_ALL_STATIC=1
          export OPENSSL_NO_VENDOR=1
          ${hiddenRust.cargo}/bin/cargo build --release --message-format short --color never
        '';

        agentCheck = pkgs.writeShellScriptBin "agent-check" ''
          echo "Checking Execution Engine..."
          export LIBCLANG_PATH=${pkgs.libclang.lib}/lib
          export LD_LIBRARY_PATH=${ldLibraryPath}${pkgs.openssl.out}/lib:${pkgs.zlib}/lib:''${LD_LIBRARY_PATH:-}
          export OPENSSL_DIR=${pkgs.openssl.dev}
          export OPENSSL_LIB_DIR=${pkgs.openssl.out}/lib
          export OPENSSL_INCLUDE_DIR=${pkgs.openssl.dev}/include
          export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:''${PKG_CONFIG_PATH:-}"
          export PKG_CONFIG_ALL_STATIC=1
          export OPENSSL_NO_VENDOR=1
          ${hiddenRust.cargo}/bin/cargo check --message-format short --color never
        '';

        agentTest = pkgs.writeShellScriptBin "agent-test" ''
          echo "Testing Execution Engine..."
          export LIBCLANG_PATH=${pkgs.libclang.lib}/lib
          export LD_LIBRARY_PATH=${ldLibraryPath}${pkgs.openssl.out}/lib:${pkgs.zlib}/lib:''${LD_LIBRARY_PATH:-}
          export OPENSSL_DIR=${pkgs.openssl.dev}
          export OPENSSL_LIB_DIR=${pkgs.openssl.out}/lib
          export OPENSSL_INCLUDE_DIR=${pkgs.openssl.dev}/include
          export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:''${PKG_CONFIG_PATH:-}"
          export PKG_CONFIG_ALL_STATIC=1
          export OPENSSL_NO_VENDOR=1
          ${hiddenRust.cargo}/bin/cargo test --message-format short --color never
        '';

        agentAdd = pkgs.writeShellScriptBin "agent-add" ''
          if [ -z "$1" ]; then echo "Usage: agent-add <crate>"; exit 1; fi

          echo "Adding dependency: $1"
          ${hiddenRust.cargo}/bin/cargo add "$@"

          echo "Auto-running Security Audit..."
          ${pkgs.cargo-audit}/bin/cargo-audit --color never
        '';

        agentFix = pkgs.writeShellScriptBin "agent-fix" ''
          echo "Attempting Auto-Fix..."
          export LIBCLANG_PATH=${pkgs.libclang.lib}/lib
          export LD_LIBRARY_PATH=${ldLibraryPath}${pkgs.openssl.out}/lib:${pkgs.zlib}/lib:''${LD_LIBRARY_PATH:-}
          export OPENSSL_DIR=${pkgs.openssl.dev}
          export OPENSSL_LIB_DIR=${pkgs.openssl.out}/lib
          export OPENSSL_INCLUDE_DIR=${pkgs.openssl.dev}/include
          export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:''${PKG_CONFIG_PATH:-}"
          export PKG_CONFIG_ALL_STATIC=1
          export OPENSSL_NO_VENDOR=1
          ${hiddenRust.cargo}/bin/cargo fix --allow-no-vcs --broken-code
        '';

        agentContext = ''
          # Restricted Environment Protocol

          ## ACCESS DENIED
          - You do **not** have access to `cargo`, `rustc`, or `clippy`.
          - Do not attempt to run them directly. It will fail.

          ## ALLOWED ACTIONS
          | Action | Command |
          | :--- | :--- |
          | Check Syntax | `agent-check` |
          | Build Release | `agent-build` |
          | Run Tests | `agent-test` |
          | Add Dependency | `agent-add <crate>` (Auto-audits) |
          | Auto-Fix Code | `agent-fix` |
        '';

        commonShellHook = ''
          export LIBCLANG_PATH=${pkgs.libclang.lib}/lib
          export LD_LIBRARY_PATH=${ldLibraryPath}${pkgs.openssl.out}/lib:${pkgs.zlib}/lib:$LD_LIBRARY_PATH
          export OPENSSL_DIR=${pkgs.openssl.dev}
          export OPENSSL_LIB_DIR=${pkgs.openssl.out}/lib
          export OPENSSL_INCLUDE_DIR=${pkgs.openssl.dev}/include
          export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
          export PKG_CONFIG_ALL_STATIC=1
          export OPENSSL_NO_VENDOR=1
        '';

      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            fenix.packages.${system}.stable.rustc
            fenix.packages.${system}.stable.cargo
            fenix.packages.${system}.stable.rust-src
            fenix.packages.${system}.stable.rust-std
            pkgs.libclang
            pkgs.llvm
            pkgs.pkg-config
            pkgs.openssl
            pkgs.openssl.dev
            pkgs.zlib
            pkgs.mdbook
          ];
          shellHook = ''
            echo "Execution Engine Dev Shell"
            ${commonShellHook}
          '';
        };

        devShells.agent-restricted = pkgs.mkShell {
          buildInputs = [
            pkgs.stdenv.cc
            pkgs.pkg-config
            pkgs.libclang
            pkgs.llvm
            pkgs.openssl
            pkgs.openssl.dev
            pkgs.zlib
            pkgs.mdbook

            agentBuild
            agentCheck
            agentTest
            agentAdd
            agentFix
          ];

          shellHook = ''
            echo "Initializing Restricted Agent Environment..."
            echo "${agentContext}" > AGENT_CONTEXT.md

            ${commonShellHook}

            if command -v cargo &> /dev/null; then
               echo "WARNING: Cargo leaked into PATH!"
            else
               echo "Cargo is successfully hidden."
            fi
          '';
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "execution-engine";
          version = "0.1.0";
          src = ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
          
          buildInputs = with pkgs; [
            openssl
            zlib
          ];
          
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
        };

        checks.build = self.packages.${system}.default;
        
        checks.clippy = pkgs.rustPlatform.buildRustPackage {
          pname = "execution-engine-clippy";
          version = "0.1.0";
          src = ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          
          nativeBuildInputs = with pkgs; [
            pkg-config
            fenix.packages.${system}.stable.cargo
            fenix.packages.${system}.stable.clippy
          ];
          
          buildInputs = with pkgs; [
            openssl
            zlib
          ];
          
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
          
          buildPhase = ''
            cargo clippy --all-targets --all-features -- -D warnings
          '';
          
          installPhase = ''
            touch $out
          '';
        };
      }
    );
}
