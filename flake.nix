{
  description = "paraphase - a build tool for data formats";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # Create patched pkgconfig files with explicit include paths
        ffmpegPkgConfig = pkgs.runCommand "ffmpeg-pkgconfig" {} ''
          mkdir -p $out/lib/pkgconfig
          for pc in ${pkgs.ffmpeg.dev}/lib/pkgconfig/*.pc; do
            name=$(basename $pc)
            # Replace empty Cflags with explicit include path
            sed 's|^Cflags:.*|Cflags: -I${pkgs.ffmpeg.dev}/include|' $pc > $out/lib/pkgconfig/$name
          done
        '';
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "paraphase";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = with pkgs; [ pkg-config llvmPackages.libclang ];
          buildInputs = with pkgs; [ ffmpeg ];
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        };

        devShells.default = pkgs.mkShell rec {
          buildInputs = with pkgs; [
            stdenv.cc.cc
            # Rust toolchain
            rustc
            cargo
            rust-analyzer
            clippy
            rustfmt
            # Fast linker for incremental builds
            mold
            clang
            # Video processing
            ffmpeg
            pkg-config
            llvmPackages.libclang.lib
            # JS tooling: docs
            bun
          ];
          LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath buildInputs}:$LD_LIBRARY_PATH";
          # Use patched pkgconfig with explicit include paths
          PKG_CONFIG_PATH = "${ffmpegPkgConfig}/lib/pkgconfig";
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        };
      }
    );
}
