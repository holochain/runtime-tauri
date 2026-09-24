# Unreleased

- Security hardening from the September 2026 unyt audit (its runtime items):
  - hc-auth no longer signs the auth server's challenge bytes as they are. Lair's
    signature is the same Ed25519 primitive Holochain uses over actions, and a
    consumer may install its hApp with the auth key, so an auth server (or
    whoever held its certificate) could have obtained a signature over a
    serialized action. `sign_challenge` now takes a decoded `hc_auth::Challenge`
    and signs `"hc-auth-challenge:"` followed by it; a challenge that is not the
    32 bytes the server issues ends the flow with `HcAuthStatus::Failed`, as an
    unreachable server does. Requests and the auth material carry
    `"scheme": "hc-auth-challenge-v1"` so hc-auth-server verifies the prefixed
    message and can tell older raw-signature clients apart. The server change
    has to be deployed before any client running this code.
  - `Runtime::ensure_app_websocket` and `setup_app` take the `AllowedOrigins` the
    app interface accepts instead of attaching with `Any`, and
    `main_window_builder` on the legacy websocket path passes the window's own
    origin. The token stays reusable and non-expiring: it is injected on every
    page load and `@holochain/client` re-authenticates with it on reconnect, so
    a single-use or expiring token would break reload and reconnect.
  - Windows from `main_window_builder` are confined to the origin they first
    load from, recorded from the webview itself rather than predicted from the
    config: navigation elsewhere is refused and logged (`blob:` URLs of the
    origin excepted, so exports still work), and `window.open` and
    `target="_blank"` open nothing. `HolochainPlugin::lock_navigation` applies
    the policy to a window the app builds itself; `navigation_allowed`,
    `origin_of` and `same_origin` are public. This needs Tauri 2.8, which added
    `on_new_window`, so the workspace floor moves from 2.5.1 to 2.8.0.
  - The Linux camera/microphone grant is answered per request and only while
    the page asking is on the origin the webview first loaded; any other page
    is denied.
  - `Runtime::ensure_app_websocket` fails with `AppInterfaceOriginsMismatch`
    when the app's cached interface was attached for different origins, instead
    of returning a port that refuses the handshake. `AppAuth` records them.
  - `get_`, `default_` and `set_user_network_config` are plugin commands now
    (`plugin:hc|…`), so Tauri's ACL applies to them. `hc:default` includes the
    two reads; `set`, which repoints bootstrap and relay and restarts, needs
    `hc:allow-set-user-network-config` on the window that hosts the settings
    screen. Apps drop them from `generate_handler!`, keep
    `.manage(UserNetworkConfigPath(..))`, and invoke them with the `plugin:hc|`
    prefix; emergence does all three when it next bumps the plugin (Rust fails
    to compile until then, the Svelte `invoke` calls fail at run time). Calling
    them without that state is `Error::UserNetworkConfigPathNotManaged` rather
    than a panic.
  - `create-holochain-tauri` ships a restrictive `csp` instead of `csp: null`:
    script from the app only plus `'wasm-unsafe-eval'` for libsodium, no remote
    or inline script, `style-src-attr 'unsafe-inline'` because Tauri's style
    nonces would otherwise cancel `'unsafe-inline'` for `style` attributes, and
    `connect-src` limited to Tauri IPC (the legacy `use_app_websocket` path also
    needs `ws://localhost:*`). Tauri only applies a CSP to pages it serves, so
    `tauri dev` against a Vite `devUrl` runs without one. The example app ships
    the same policy, which its dev build does apply, and turns `withGlobalTauri`
    off, invoking its `report` command through `@tauri-apps/api` instead.
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
- Linux: `tauri-plugin-hc` grants WebKitGTK user-media permission requests on
  every webview it sees, so a hApp UI can call `getUserMedia` (camera and
  microphone) on Linux as it already could on Android. WebKitGTK denies these
  unless the embedder answers its `permission-request` signal, and neither wry
  nor Tauri does. The dev shell also adds `gst-plugins-good` and exports
  `GST_PLUGIN_PATH_1_0`: the nix webkit's own GStreamer closure carries no
  capture device provider, so it enumerated zero cameras and `getUserMedia`
  failed with `OverconstrainedError: Invalid constraint` whatever the constraints.
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
