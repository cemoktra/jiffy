{
  description = "Jiffy: a local-only terminal forked from Warp, with jj and Claude Code support (Linux-only).";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      rust-overlay,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          lib = pkgs.lib;
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };
          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
          appCargoToml = builtins.fromTOML (builtins.readFile ./app/Cargo.toml);
          version = "${appCargoToml.package.version}+${self.shortRev or "dirty"}";
          src = self;
          cargoLock = ./Cargo.lock;
          # Match Zed's approach: derive the main app vendor directory from
          # Cargo.lock instead of maintaining a top-level cargoVendorHash.
          cargoVendorDir =
            let
              craneVendorDir = craneLib.vendorCargoDeps {
                inherit src cargoLock;
                overrideVendorGitCheckout =
                  crates: drv:
                  drv.overrideAttrs (old: {
                    postPatch = (old.postPatch or "") + ''
                      find . -name 'Cargo.toml.orig' -delete
                    '';
                  });
              };
            in
            # crane writes a root config.toml; buildRustPackage expects the
            # cargoDeps layout to include .cargo/config.toml and Cargo.lock.
            pkgs.runCommand "jiffy-${version}-cargo-vendor" { } ''
              cp -R ${craneVendorDir}/. "$out"
              chmod u+w "$out"
              mkdir -p "$out/.cargo"
              sed 's|${craneVendorDir}|@vendor@|g' \
                "$out/config.toml" > "$out/.cargo/config.toml"
              rm "$out/config.toml"
              cp ${cargoLock} "$out/Cargo.lock"
            '';

          linuxRuntimeLibraries = with pkgs; [
            curl
            dbus
            expat
            fontconfig
            freetype
            libGL
            libgit2
            libxkbcommon
            openssl
            stdenv.cc.cc.lib
            udev
            vulkan-loader
            wayland
            libx11
            libxscrnsaver
            libxcursor
            libxext
            libxfixes
            libxi
            libxrandr
            libxrender
            libxcb
            zlib
          ];

          buildFeatures = [
            "release_bundle"
            "gui"
          ];

          jiffy = rustPlatform.buildRustPackage {
            pname = "jiffy";
            inherit version;

            inherit src;
            cargoDeps = cargoVendorDir;

            nativeBuildInputs = with pkgs; [
              cargo-about
              clang
              cmake
              jq
              mold
              makeWrapper
              patchelf
              pkg-config
              python3
            ];

            buildInputs = linuxRuntimeLibraries;

            cargoBuildFlags = [
              "-p"
              "jiffy"
              "--bin"
              "jiffy"
            ];
            inherit buildFeatures;

            # The application test suite is large and GUI/integration-heavy; this
            # flake's package check is the Nix build plus a launch smoke test.
            doCheck = false;

            env = {
              APPIMAGE_NAME = "Jiffy-${pkgs.stdenv.hostPlatform.parsed.cpu.name}.AppImage";
              LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
              CARGO_PROFILE_RELEASE_DEBUG = "false";
            };
            postInstall =
              let
                installDir = "$out/opt/jiffy";
                resourcesDir = "${installDir}/resources";
                releaseChannel = "oss";
                libraryPath = lib.makeLibraryPath linuxRuntimeLibraries;
                executablePath = lib.makeBinPath (with pkgs; [ xdg-utils ]);
              in
              ''
                install -Dm755 "$out/bin/jiffy" "${installDir}/jiffy"
                rm -f "$out/bin/jiffy"

                patchShebangs \
                  ./script/prepare_bundled_resources \
                  ./script/copy_conditional_skills

                SETTINGS_SCHEMA_EXECUTABLE="${installDir}/jiffy" ./script/prepare_bundled_resources \
                  "${resourcesDir}" \
                  "${releaseChannel}"

                install -Dm644 \
                  "${resourcesDir}/THIRD_PARTY_LICENSES.txt" \
                  "$out/share/licenses/jiffy/THIRD_PARTY_LICENSES.txt"

                install -Dm644 LICENSE-AGPL "$out/share/licenses/jiffy/LICENSE-AGPL"
                install -Dm644 LICENSE-MIT "$out/share/licenses/jiffy/LICENSE-MIT"

                install -Dm644 app/channels/oss/dev.jiffy.Jiffy.desktop \
                  "$out/share/applications/dev.jiffy.Jiffy.desktop"
                for size in 16x16 32x32 64x64 128x128 256x256 512x512; do
                  icon="app/channels/oss/icon/no-padding/$size.png"
                  if [ -f "$icon" ]; then
                    install -Dm644 "$icon" \
                      "$out/share/icons/hicolor/$size/apps/dev.jiffy.Jiffy.png"
                  fi
                done

                wrapProgram "${installDir}/jiffy" \
                  --prefix LD_LIBRARY_PATH : "${libraryPath}" \
                  --prefix PATH : "${executablePath}"

                mkdir -p "$out/bin"
                ln -s "${installDir}/jiffy" "$out/bin/jiffy"
              '';

            postFixup = lib.optionalString pkgs.stdenv.isLinux ''
              wrapped="/opt/jiffy/.jiffy-wrapped"
              if [ -e "$out$wrapped" ] && ! patchelf --print-needed "$out$wrapped" | grep -q '^libfontconfig\.so\.1$'; then
                patchelf --add-needed libfontconfig.so.1 "$out$wrapped"
              fi
            '';

            meta = {
              description = "Jiffy: a local-only terminal forked from Warp, with jj and Claude Code support (Linux-only).";
              license = lib.licenses.agpl3Only;
              mainProgram = "jiffy";
              platforms = systems;
              sourceProvenance = with lib.sourceTypes; [ fromSource ];
            };
          };
        in
        {
          inherit jiffy;
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          lib = pkgs.lib;
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          nativeBuildInputs = with pkgs; [
            cargo-about
            cargo-nextest
            clang
            cmake
            jq
            lld
            mold
            makeWrapper
            patchelf
            pkg-config
            python3
            rustToolchain
            rust-analyzer
          ];
          buildInputs = with pkgs; [
            curl
            dbus
            expat
            fontconfig
            freetype
            libGL
            libgit2
            libxkbcommon
            openssl
            stdenv.cc.cc.lib
            udev
            vulkan-loader
            wayland
            libx11
            libxscrnsaver
            libxcursor
            libxext
            libxfixes
            libxi
            libxrandr
            libxrender
            libxcb
            zlib
          ];
        in
        {
          default = pkgs.mkShell {
            inherit nativeBuildInputs buildInputs;
            APPIMAGE_NAME = "Jiffy-${pkgs.stdenv.hostPlatform.parsed.cpu.name}.AppImage";
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
          };
        }
      );

      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixfmt);
    };
}
