# Unreleased

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
- The repository's JavaScript tooling moves from pnpm to npm workspaces, matching
  the npm/Yarn-classic tooling the rest of the Holochain app ecosystem uses. The
  example UI, which was installed with `npm --prefix` outside the pnpm workspace
  and had no committed lockfile, is now a workspace covered by the root
  `package-lock.json`. The generated Android Gradle task and iOS Xcode build phase
  invoke `npm run tauri -- …` instead of `pnpm tauri …`.
- BREAKING: the plugin crate is renamed `tauri-plugin-holochain` → `tauri-plugin-hc`
  (the former name is reserved on crates.io by another owner), and its Tauri
  identifier follows: `holochain:default` → `hc:default`,
  `holochain:allow-sign-payload` → `hc:allow-sign-payload`, and commands are
  invoked as `plugin:hc|…`. The identifier has to match the `links` key, which
  `tauri-build` uses to name the plugin's permissions. Rust imports become
  `tauri_plugin_hc::…`. The injected `__HC_TAURI_HOLOCHAIN__` env (whose
  `PLUGIN_NAME` now carries `hc`, which `@holochain/client` reads) and the
  `holochain://` event names are unchanged.
- The repository is now scoped to the cross-platform Tauri runtime: the
  `holochain-conductor-runtime` crate, the in-process `tauri-plugin-holochain`,
  and the example app for desktop, Android and iOS. The Android
  conductor-as-a-foreground-service stack (the service and client plugins, the
  UniFFI bindings, and the Kotlin libraries) now lives in
  [holochain/android-service-runtime](https://github.com/holochain/android-service-runtime).
- iOS support merged: the example app builds and runs on device and simulator
  under holochain's `wasmi` backend, selected by target cfg. Requires an
  in-process lair keystore that does not bind a unix socket, which is not yet
  upstream — see `docs/ios-test-build-plan.md`.
- BREAKING: `holochain-conductor-runtime` drops `AuthorizedAppClientsManager`,
  `ClientId`, `AutostartConfigManager`, the `Persisted` trait, and
  `Runtime::authorize_app_client` / `is_app_client_authorized`. These gated
  cross-app access by Android package name and persisted a start-on-boot flag
  for the foreground service; they have no meaning in an in-process runtime and
  move with the Android stack.
- `tauri-plugin-holochain` no longer depends on
  `holochain-conductor-runtime-types-ffi`. It built `ZomeCallParams` by way of
  the UniFFI types purely to reuse a conversion, which pulled UniFFI into an
  otherwise FFI-free dependency tree. The conversion is now inline and reports
  a malformed nonce, cap secret, provenance or cell id as
  `Error::Serialization` instead of panicking.

# 0.3.0

- BREAKING: `HolochainPlugin::try_runtime` reports a failed conductor boot as the new `Error::SetupFailed`, carrying the cause, instead of `Error::NotReady` forever. `holochain://setup-failed` still fires with the same payload.
- `Runtime::sign_payload` signs an arbitrary payload with a caller-chosen agent key via the keystore, for protocols beyond zome calls that need proof of control over an identity. Exposed through the in-process plugin as the `sign_payload` command, returning a base64-encoded signature. It is outside `holochain:default`, so a capability has to grant `holochain:allow-sign-payload` itself.
- `Runtime::install_app` preserves the typed `ConductorError` (as `RuntimeError::Conductor`) instead of flattening it to a string.
- Bump to holochain 0.7.0. New conductor types are carried across the FFI: `AppStatusFfi` gains `AwaitingRestore` and `Unrecoverable` (cell id + rendered reason); `InstallAppPayload.restore_from_dht` and `RoleSettings::Provisioned.init_properties` are not exposed over FFI yet (always `false`/`None`).
- BREAKING: `RuntimeNetworkConfig` / `RuntimeNetworkConfigFfi` lose `signal_url` and `ice_urls` — holochain 0.7 dropped the tx5/WebRTC transport in favor of iroh, so the conductor `NetworkConfig` no longer carries them. Only `bootstrap_url` and `relay_url` remain.
- The forum test fixture is repacked with hdk 0.7.0 / hdi 0.8.0 (the 0.7 `Action`/`ActionData` restructure required porting the coordinator zome's signal emission).
- Bump to holochain 0.6.3.
- In-place window rebind: `HolochainPlugin::rebind_window` moves a webview window between installed apps (or to app-less) without recreating the OS window, emitting `holochain://rebound` so the injected env re-points `@holochain/client` to the new app. A failed rebind keeps the prior binding (no half-rebind), destroyed windows are pruned (routing + signal forwarder), and a monotonic `seq` keeps the rebound event order-safe.
- The 'Open Settings' button will first attempt to open the android-service-runtime app via the system settings (i.e. for 'system' builds). If that fails it will fallback to opening the app via the launcher (i.e. for 'user' builds).

# 0.2.2
- bump to holochain 0.5.6
- fix example client app connection bug
- fix invalid bootstrap url
- move settings link from homepage to system settings

# 0.2.1
- bump to holochain 0.5.4
- fix infrastructure urls

# 0.2.0
- CI to publish kotlin libraries to Maven Central, triggered by a release tag.
- Specify ICE Servers in `RuntimeConfigFfi`
- Add support for translations, with only english locale written.
- CI to publish tauri-plugin-holochain-service, tauri-plugin-holochain-service-client and holochain-conductor-runtime-types-ffi to crates.io, triggered by release tag.
- The runtime network configuration must now be specified when initializing `tauri-plugin-holochain-service`. Previously, it was hard-coded.
- Upgrade to holochain 0.5
- Revert back to using p2p-shipyard nix flake

# 0.1.0
- Replace 3rd party `holochain_runtime` with `holochain-conductor-runtime`, modify FFI-bindings wrapper crate to use it. The runtime now makes calls via the `ConductorHandle` directly, without going through an `AdminWebsocket` to prevent unauthenticated access.
- Rename the FFI-bindings wrapper crate to `holochain-conductor-runtime-ffi` and generated kotlin bindings have been renamed accordingly.
- Removed the call `get_admin_port`, as an `AdminWebsocket` is no longer exposed.
- Split out types from holochain-conductor-runtime-ffi into separate crate holochain-conductor-runtime-types-ffi
- Create standalone kotlin library containing the types and client for binding and calling the HolochainService: `org.holochain.androidserviceruntime.holochain_service_client`
- Gradle setup for publishing standalone kotlin library to Maven Central
- Use published release in both tauri-plugin-holochain-service and tauri-plugin-holochain-service-consumer
- Minor fixes for android-service-runtime to get it working again
- Add function `isReady` to check that the conductor is available, expose in client and tauri-plugin-holochain-service
- Move uniffi-bindgen cli tool for generating bindings into its own crate, remove from *-ffi crates (see https://mozilla.github.io/uniffi-rs/0.27/tutorial/foreign_language_bindings.html#running-uniffi-bindgen-using-a-library-file-recommended)
- Commands and setup for publishing kotlin library to local maven repo. All other projects will check local maven repo *before* Maven Central.
- Android app logs warnings and errors to android system log.
- The following HolochainService functions exposed via IPC are now async and do not block the main thread: listApps, installApp, uninstallApp, enableApp, disableApp, isAppInstalled, ensureAppWebsocket, and signZomeCall.
- Run holochain-service-client tests in CI
- Fix Ffi types to Parcelable types converstions, and tests for Parcelable types.
- Support Json serialization of sealed classes, cleanup and tests of Json serialization.
- Added example app demonstrating use of tauri-plugin-holochain-service-consumer
- Fix inconsistent crashes on relaunch with "logger already initialized" errors.
- Add nix flake for android + holochain development, remove reliance on p2p-shipyard flake.
- Added example app demonstrating use of tauri-plugin-holochain-service-consumer
- Ensure the __HC_LAUNCHER_ENV__ is defined before the webview is initialized, by moving the app setup logic from injected JS in the webview to rust run during tauri setup.
- Add command `setupApp` to `HolochainServiceClient` that includes all logic for installing an app if necessary, enabling it, and setting up the app ws authentication.
- Remove commands `installApp`, `connect`, `isAppInstalled`, `ensureAppWebsocket` from `tauri-plugin-holochain-service-consumer`. Their function has been replaced by a single command `setupApp`.
- Display notice on consumer app launch when unable to connect to HolochainService.
- Refactor setupApp implementation by moving core logic into the `holochain-service-runtime` crate, rename `tauri-plugin-holochain-service-consumer` command from `setupApp` to `connectSetupApp`.
- Split HolochainService IPC binders into "admin" and "app" binders. Restrict admin binder calls to only the same android package as the service. Restrict app binder calls to only authorized package + happ pairs. Currently new package + happ pairs are authorized automatically.
- Extract HolochainService into standalone kotlin library `org.holochain.androidserviceruntime.holochain_service` to simplify testing. Run tests in CI.
- Re-added ktlint for kotlin linting
- Rename kotlin packages to resolve lint complaints
- Rename library and crate directories to reduce noise
- Android apps now require manual user approval to connect to a new holochain app
- Clearer npm package commands
- HolochainService will now auto start on system boot, if it was previously running during the last system shutdown.
- Fix bug in `apps/android-service-runtime`, where the app would crash if the service was not running when switching to the "My Apps" tab.
- Lint all kotlin libraries in CI
- Build android-service-runtime and example-client-app in CI
- Build html docs for both kotlin libraries, and publish to github pages, in CI.
