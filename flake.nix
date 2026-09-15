{
  description = "Holochain Tauri Runtime — development environment";

  # We depend on holonix only for the Holochain toolchain (holochain, hc,
  # hc-scaffold, lair-keystore, ...) and the rust-overlay it already pins.
  # Everything else (Rust toolchain, Android SDK/NDK, Tauri desktop libs) is
  # composed here from nixpkgs, so the repo has no dependency on any external
  # Tauri/Holochain dev-shell flake.
  inputs = {
    holonix.url = "github:holochain/holonix/main-0.7";

    nixpkgs.follows = "holonix/nixpkgs";
    flake-parts.follows = "holonix/flake-parts";
    rust-overlay.follows = "holonix/rust-overlay";
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = builtins.attrNames inputs.holonix.devShells;
      perSystem = { system, inputs', ... }:
        let
          pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ (import inputs.rust-overlay) ];
            config = {
              allowUnfree = true; # Android SDK/NDK are unfree
              android_sdk.accept_license = true;
            };
          };

          # Rust toolchain. Channel, components, and cross-compilation targets all
          # come from ./rust-toolchain.toml so nix and rustup users stay in sync.
          rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

          # Android SDK + NDK, matching the gradle config (compileSdk/buildTools 34,
          # minSdk 27). The NDK is what `cargo ndk` uses to cross-compile the Rust
          # crates into the jniLibs consumed by the Android libraries/plugins.
          # r28: clang 19 — needed because vendored OpenSSL 3.6 (libsqlite3-sys →
          # sqlcipher) ships SM4 x86 asm that r26's clang 17 can't assemble — and
          # .so files come out 16 KB-page-aligned by default (Play requirement for
          # apps targeting Android 15+).
          ndkVersion = "28.2.13676358";
          androidComposition = pkgs.androidenv.composeAndroidPackages {
            # 34: the apps' compileSdk/targetSdk. 36: tauri 2.11's bundled
            # `:tauri-android` gradle subproject compiles against it, and gradle
            # cannot auto-install platforms into the read-only nix store.
            platformVersions = [ "34" "36" ];
            buildToolsVersions = [ "34.0.0" ];
            includeNDK = true;
            ndkVersions = [ ndkVersion ];
            cmakeVersions = [ "3.22.1" ];
            includeEmulator = false; # emulator is provided by CI / installed on demand
            includeSystemImages = false;
          };
          androidSdk = androidComposition.androidsdk;
          androidHome = "${androidSdk}/libexec/android-sdk";
          ndkHome = "${androidHome}/ndk/${ndkVersion}";

          # System libraries to build/run a Tauri v2 desktop app on Linux. Needed
          # for the desktop-first unified plugin work; harmless for Android builds.
          tauriDeps = with pkgs; [
            webkitgtk_4_1
            gtk3
            gdk-pixbuf
            glib
            glib-networking
            librsvg
            libsoup_3
            dbus
            openssl
          ];

          # Holochain tools every app shell needs at runtime or for zome builds.
          holochainTools = with inputs'.holonix.packages; [
            holochain
            hc
            lair-keystore
            bootstrap-srv
          ];

          # Everything to build and run a Tauri v2 desktop app with an in-process
          # conductor, and to build zomes (the Rust toolchain includes wasm32).
          # Deliberately no Node: apps choose their own version.
          tauriPackages = holochainTools ++ [ rust ] ++ (with pkgs; [
            cmake # aws-lc-sys (iroh/rustls crypto) builds its C sources with CMake
            pkg-config
            binaryen # wasm-opt, for building hApp/zome wasm
            shared-mime-info
            gsettings-desktop-schemas
          ]);

          tauriShellHook = ''
            # TLS for the nix webkit (glib-networking's GIO module). Additive on
            # purpose: GIO_MODULE_DIR would override the module search path for
            # every GLib app launched from this shell.
            export GIO_EXTRA_MODULES=${pkgs.glib-networking}/lib/gio/modules
            # webkitgtk >= 2.44 requires a working EGL display in its web process
            # (DMA-BUF renderer + Skia) and aborts with EGL_BAD_PARAMETER without
            # one. nixpkgs' libglvnd only searches /run/opengl-driver for EGL
            # vendor drivers — a NixOS-only path — so on other distros the nix
            # dev shell finds no GPU driver and the webview dies blank. Supply a
            # vendor list: the host's native drivers first (host binaries run
            # from this shell behave exactly as outside it), then nixpkgs Mesa,
            # which nix-linked binaries fall back to (llvmpipe here on NVIDIA;
            # can drive AMD/Intel GPUs directly) after failing to dlopen the
            # host's driver. NixOS hosts skip this and keep /run/opengl-driver.
            if [ ! -e /run/opengl-driver ] && [ -z "$__EGL_VENDOR_LIBRARY_FILENAMES" ] && [ -z "$__EGL_VENDOR_LIBRARY_DIRS" ]; then
              export __EGL_VENDOR_LIBRARY_DIRS=/etc/glvnd/egl_vendor.d:/usr/share/glvnd/egl_vendor.d:${pkgs.mesa}/share/glvnd/egl_vendor.d
            fi
            # GTK schema lookup for the nix webkit; GSETTINGS_SCHEMAS_PATH is
            # filled by the glib setup hook from the schemas in `packages`.
            export XDG_DATA_DIRS=$GSETTINGS_SCHEMAS_PATH:$XDG_DATA_DIRS
          '';

          androidPackages = [ androidSdk ] ++ (with pkgs; [
            cargo-ndk # build Rust -> Android jniLibs
            jdk17 # Gradle
          ]);

          androidShellHook = ''
            export ANDROID_HOME="${androidHome}"
            export ANDROID_SDK_ROOT="${androidHome}"
            export ANDROID_NDK="${ndkHome}"
            export ANDROID_NDK_ROOT="${ndkHome}"
            export ANDROID_NDK_HOME="${ndkHome}"
            export NDK_HOME="${ndkHome}"

            # cargo-ndk exports plain CC/CXX/AR pointing at the NDK clang, which
            # also hijacks *host* compiles (build scripts, proc-macro deps — e.g.
            # sqlx-macros' vendored OpenSSL). The HOST_* variants take precedence
            # in the `cc` crate for host-targeted units, so host builds keep the
            # host toolchain even under `cargo ndk`.
            export HOST_CC=gcc
            export HOST_CXX=g++
            export HOST_AR=ar
          '';
        in
        {
          # Shells for apps built on tauri-plugin-hc to compose, e.g.
          #   devShells.default = pkgs.mkShell {
          #     inputsFrom = [ inputs'.tauri-runtime.devShells.tauriDev ];
          #     packages = [ pkgs.nodejs_24 ];
          #   };
          # tauriDev: desktop app + zome builds. tauriAndroidDev: adds the
          # Android SDK/NDK, JDK and cargo-ndk. Neither sets PS1 or provides Node.
          devShells.tauriDev = pkgs.mkShell {
            packages = tauriPackages;
            buildInputs = tauriDeps; # libraries to link against, via pkg-config
            shellHook = tauriShellHook;
          };

          devShells.tauriAndroidDev = pkgs.mkShell {
            packages = tauriPackages ++ androidPackages;
            buildInputs = tauriDeps;
            shellHook = tauriShellHook + androidShellHook;
          };

          # This repository's own shell: desktop + Android, plus Node for the
          # plugin's JS bundle and the example UI, and hc-scaffold for fixtures.
          devShells.default = pkgs.mkShell {
            packages = tauriPackages ++ androidPackages ++ [
              inputs'.holonix.packages.hc-scaffold
              pkgs.nodejs_22
            ];
            buildInputs = tauriDeps;
            shellHook = tauriShellHook + androidShellHook + ''
              # Visual cue that you are inside the dev shell.
              export PS1='\[\033[1;35m\][tauri-runtime:\w]\$\[\033[0m\] '
            '';
          };
        };
    };
}
