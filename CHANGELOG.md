# Unreleased

- The Makefile is gone; `npm run ci` is what CI runs (`fmt:check`, `lint`, `test`).
  `npm run lint` now builds the example UI first, since `cargo clippy --workspace`
  compiles the example app and Tauri resolves `frontendDist` at compile time. CI
  is one workflow ending in a `ci_pass` job, the status check the holochain org
  rulesets require.
- `create-holochain-tauri` (`npm create holochain-tauri`) adds a desktop and Android
  app to an hApp repository: `src-tauri/` using the startup helpers, `package.json`
  scripts for desktop and Android agents on a local dev network, and dev shells on
  runtime-tauri's. It rewrites `flake.nix` only when it is `hc-scaffold`'s and
  otherwise leaves it for the manual steps in its README. `init` only; no `update`
  yet.
- Startup helpers, extracted from Emergence so each app stops carrying its own
  copy: `tauri_plugin_hc::app_paths` (production and per-dev-instance data
  directories), `UserNetworkConfig` with the `get_`/`default_`/`set_user_network_config`
  app commands, and `on_ready`, which runs startup work once the conductor is up
  even if it came up first. `holochain-conductor-runtime` adds
  `Runtime::install_app_if_missing`, which `setup_app` now uses, and fails boot
  with `DataRootPathTooLong` when lair's socket path would exceed the Unix socket
  limit instead of lair's `path must be shorter than SUN_LEN`.
- `tauri-plugin-hc` adds `dev_network_config(url)` and `dev_network_url!()` for
  local dev networks: one `kitsune2-bootstrap-srv` as bootstrap server and iroh
  relay, plain-HTTP relay allowed, and kitsune2's gossip `initiateBurstFactor`
  raised from 3 to 100. With the default, a phone agent dropped a desktop
  agent's gossip rounds as over the limit and new data took minutes to arrive;
  with 100 there were no drops and a fresh phone agent synced in about 90 s.
  `dev_network_url!()` reads `INTERNAL_IP`/`BOOTSTRAP_PORT` at run time on desktop
  and at compile time on mobile.
- Windows from `main_window_builder` are confined to the origin they first
  load from, recorded from the webview itself rather than predicted from the
  config: navigation elsewhere is refused and logged (`blob:` URLs of the
  origin excepted, so exports still work), and `window.open` and
  `target="_blank"` open nothing. `HolochainPlugin::lock_navigation` applies
  the policy to a window the app builds itself; `navigation_allowed`,
  `origin_of` and `same_origin` are public. This needs Tauri 2.8, which added
  `on_new_window`, so the workspace floor moves from 2.5.1 to 2.8.0.
- Linux: `tauri-plugin-hc` grants WebKitGTK user-media permission requests on
  every webview it sees, so a hApp UI can call `getUserMedia` (camera and
  microphone) on Linux as it already could on Android. WebKitGTK denies these
  unless the embedder answers its `permission-request` signal, and neither wry
  nor Tauri does. The dev shell also adds `gst-plugins-good` and exports
  `GST_PLUGIN_PATH_1_0`: the nix webkit's own GStreamer closure carries no
  capture device provider, so it enumerated zero cameras and `getUserMedia`
  failed with `OverconstrainedError: Invalid constraint` whatever the constraints.
- The Linux camera/microphone grant is answered per request and only while
  the page asking is on the origin the webview first loaded; any other page
  is denied.
- Android: `tauri-plugin-hc` initializes `ndk_context` in `JNI_OnLoad` with the
  process's `Application`. tao 0.35 (Tauri 2.11) stopped doing this, so the first
  `ndk_context::android_context()` call panicked and the app aborted on launch;
  `app_dirs2`, iroh's DNS resolver and `netdev` all make that call. An app that
  defines its own `JNI_OnLoad` will now fail to link and should initialize
  `ndk_context` from it instead.
- Dev builds drop dependency debug info (`debug = false` beside `opt-level = 3`).
  An Android debug library was ~1.3 GB, 1.18 GB of it DWARF, and failed to
  install on an emulator. The plugin README's recommended block includes it.
- Dev and test builds optimize dependencies (`[profile.dev.package."*"] opt-level = 3`
  in the workspace root). Unoptimized, a new cell's first zome call took ~30 s and
  conductor boot ~20 s; optimized, ~3 s and under 2 s. The plugin README tells app
  developers to set the same, since profiles only apply in a workspace root.
- iOS: the example app builds and runs on device and simulator
  under holochain's `wasmi` backend, selected by target cfg. Requires an
  in-process lair keystore that does not bind a unix socket, which is not yet
  upstream — see `docs/ios-test-build-plan.md`.
