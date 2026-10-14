# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[[0.1.0](https://github.com/holochain/runtime-tauri/commits/v0.1.0-rc.0)\] - 2026-10-14

### Features

- Support holochain 0.7.1
- Support holochain 0.7.1-rc.2 by @mattyg in [#28](https://github.com/holochain/runtime-tauri/pull/28)
- Add create-holochain-tauri to add a Tauri app to an hApp repository by @zippy
  - Npm create holochain-tauri, run in an hApp repository such as one from hc-scaffold, writes src-tauri/ (a lib.rs on the plugin's startup helpers, capabilities, tauri.conf.json, icons), adds package.json scripts for desktop and Android agents on a local dev network, and bases the dev shells on runtime-tauri's. flake.nix is rewritten only when it is exactly hc-scaffold's; any other flake is left alone and the README gives the change to make by hand.
  - Runtime-tauri points the generated app at a local checkout until this repository is published. Only init exists so far; update, CI workflows and iOS are not generated yet.
- *(plugin)* Add app_paths, user network config commands and on_ready by @zippy
  - Startup helpers extracted from Emergence, which carried its own copy of each:
  - App_paths: the conductor data directory and saved network settings file,   in the user data directory in production, or one holochain-<n> directory per   running dev instance, claimed with a lock file. - UserNetworkConfig and the get_/default_/set_user_network_config app commands:   bootstrap and relay URLs saved from a settings screen, applied over the app's   network config. - on_ready: run startup work once the conductor is up, exactly once, including   when it came up before on_ready was called, which a bare EVENT_READY   listener misses.
  - None of them owns the Tauri builder, so apps keep full control of startup.
- *(runtime)* Reject a data root too long for lair's socket by @zippy
  - Holochain 0.7's in-process keystore binds a Unix socket at <data_root>/socket. A data root whose socket path exceeds the platform limit (107 bytes on Linux and Android, 103 on Apple) failed deep in lair with "path must be shorter than SUN_LEN". Boot now checks first and fails with DataRootPathTooLong, naming the path and both lengths.
- *(runtime)* Add install_app_if_missing by @zippy
  - Install an app unless it is already installed, and enable it after a fresh install. It is setup_app without attaching an app websocket, for apps whose UI reaches the conductor over in-process IPC; setup_app now uses it. The outcome says whether the app was installed or was already there.
- *(plugin)* Add dev_network_config and dev_network_url! for local dev networks by @zippy
  - Apps using a local kitsune2-bootstrap-srv were each hand-writing the same network config: one URL for bootstrap and relay, relayAllowPlainText, and INTERNAL_IP/BOOTSTRAP_PORT read at run time on desktop but compile time on mobile. Getting the host wrong on one side silently splits the network.
  - The helper also raises kitsune2's gossip initiateBurstFactor from 3 to 100. On the default, a phone agent dropped a desktop agent's gossip rounds as over the burst limit and new data took minutes to arrive; with 100 there were no drops and a fresh phone agent synced in about 90 s. Why the desktop kept initiating at the fast rate is not yet established, so this is dev only.
- *(linux)* Let hApp UIs use the camera and microphone by @zippy
  - WebKitGTK denies getUserMedia unless the embedder answers the webview's permission-request signal, and neither wry nor Tauri does (wry handles it on Android). The plugin now grants user-media requests on every webview it sees from its on_webview_ready hook, and enables media streams in the webview settings. The webkit2gtk dependency matches Tauri's own.
  - The nix dev shell also gains gst-plugins-good and exports GST_PLUGIN_PATH_1_0. The nix webkit's GStreamer closure carries only the core and base plugin sets, so it enumerated zero capture devices and getUserMedia failed with OverconstrainedError "Invalid constraint" whatever the constraints. The variable is additive so host GStreamer apps launched from the shell keep their own plugins.
- Drop the iOS path workarounds via a socket-free lair keystore
  - Proves and consumes the fix for the SUN_LEN blocker recorded in docs/ios-test-build-plan.md issue A.
  - Holochain's "in-process" lair keystore was running a StandaloneServer, which binds a unix socket at <data_root>/socket. AF_UNIX caps paths at ~104 bytes and an iOS app-container data dir is 158 B (device) to 236 B (simulator), so the conductor could not boot from any conventional location. This branch had been working around that with two ugly data_dir() arms -- /tmp/hcex in the simulator (writes outside the sandbox) and home_dir()/hc on device (~10 bytes of headroom, data at the container root, untested).
  - Both are now deleted. data_dir() is a single #[cfg(mobile)] arm using app_data_dir(), identical to Android, and the target_abi = "sim" split is gone. Net -39 lines.
  - What makes that possible is a one-file change to holochain_keystore, on branch `ios-inproc-keystore` in a sibling ../holochain checkout (cut from the holochain-0.7.0 tag): spawn_lair_keystore_in_proc now runs lair through InProcKeystore, which talks over in-memory channels and never binds. The pid-check StandaloneServer provided is retained by calling lair_keystore::pid_check::pid_check directly, so the only dropped behaviour is the socket -- which matters, because two conductors sharing a lair root is a real scenario on desktop.
  - Verified on an iPhone 17 Pro simulator: conductor boots from the standard app_data_dir() -- a 229-byte root whose socket would have been 236 B -- installs the forum hApp and completes a zome call, signing and an app signal. 0 SUN_LEN errors, and no `socket` file in the lair root (pid_file, store_file and databases all still there). 6/6 tauri-plugin-holochain desktop tests pass, both iOS targets and `cargo check --workspace --all-targets` build clean, and per-target wasm backends are unchanged.
  - [patch.crates-io] pins all 25 holochain-repo crates in our graph, not just holochain_keystore: they path-depend on each other, so patching one in isolation pulls a second copy of holo_hash into the graph and trait impls stop matching across the two ("no method named `sign_raw` found for struct `HoloHash<T>`").
- Prepare iOS device build (path budget, local-network usage string)
  - Follow-up to a0dfe96. Adds the pieces a physical-device run needs, so that step is a signing exercise rather than a code change. NEITHER CHANGE HAS RUN ON HARDWARE -- no signing identity was available on this machine.
  - Data_dir(): split the iOS arm on target_abi = "sim". The simulator's app container is ~70 bytes longer than a device's, so the two cannot share a path. Measured socket-path budget against the ~104 B AF_UNIX limit, using a real 36-char container UUID and counting lair's canonicalization (/var -> /private/var):
  - Device     <container>/hc/socket             94 B  ok   device     <container>/Documents/hc/socket  104 B  fails   device     app_data_dir()/holochain/socket  158 B  fails   simulator  <container>/hc/socket            172 B  fails
  - So the device arm uses home_dir()/hc (Tauri resolves home_dir() to the app container on iOS, via the desktop path resolver) and the simulator keeps /tmp/hcex. Ten bytes of headroom is a probe, not a fix.
- Holochain-runtime-example builds and runs on iOS (simulator)
  - Phases 1-4.1 of the iOS test build plan: the unified in-process tauri-plugin-holochain cross-compiles for aarch64-apple-ios and aarch64-apple-ios-sim, and on an iPhone 17 Pro simulator (iOS 26.5) the conductor boots, the forum fixture installs, and a zome call + signal execute -- the first wasm execution under wasmi on iOS.
  - Wasm backend (differed from the plan's prediction): - Selected by target cfg in crates/runtime, not a cargo feature pair.   `tauri ios dev/build` expose --features but no --no-default-features, and   the cargo invocation lives inside the generated Xcode build phase, so a   feature-based default would have linked the JIT into the iOS binary with   no way to switch it off. Making `default` empty instead breaks   `cargo check --workspace`, which cannot pass per-package features. - iOS gets wasmer-wasmi, everything else wasmer-sys-cranelift. Declaring it   once in the deepest crate is enough since cargo unifies features. Verified   per target with `cargo tree`; the iOS build log has zero cranelift refs. - Workspace root drops holochain's default features so the backend is not   pinned there; members re-add encryption + schema.
- Sign an arbitrary payload with a caller-chosen agent key by @zo-el
- Preserve the typed ConductorError from install_app by @zo-el
  - Route `Runtime::install_app` through the conductor handle directly (`install_app_bundle`) instead of `AdminInterfaceApi::handle_request`, which flattens a conductor error into a print-only `ExternalApiWireError::InternalError(String)` (holochain TODO B-01506).
  - Going direct preserves the typed `ConductorError`, which the existing `From<ConductorError>` impl surfaces as `RuntimeError::Conductor(..)`, so callers can match on the actual failure, for instance the benign re-install case as `ConductorError::AppAlreadyInstalled`, instead of grepping a Debug string. Previously every install failure arrived as the opaque `RuntimeError::AdminApiBadResponse`.
  - The success path mirrors the admin `InstallApp` handler (`get_dna_definitions` + `AppInfo::from_installed_app`), and `check_running()` is preserved for fast-fail parity with the sibling `req_admin_api` calls. `tauri-plugin-holochain` re-exports `RuntimeError` and `ConductorError` so plugin consumers can destructure the error without depending on the runtime crate directly.
  - Add `test_install_app_already_installed`, covering that the typed conductor error survives the passthrough and that a failed re-install leaves the original app untouched.
- Unified tauri-plugin-holochain builds and runs on Android by @zippy
  - Phase 2 of the 0.7 update plan: the in-process plugin cross-compiles under the NDK and holochain-runtime-example runs as an Android app, verified end-to-end on an x86_64 API-34 emulator (conductor boot, forum happ install, direct-IPC zome call + signing, app signal).
  - Toolchain (the actual NDK work — differed from the plan's prediction): - flake: NDK 26.1 -> r28.2. aws-lc-sys (iroh) built fine; the real blocker   was vendored OpenSSL 3.6 (sqlcipher), whose SM4 x86 asm needs clang 18+.   r28 also emits 16 KB-page-aligned .so files (Play requirement). - flake: pin HOST_CC/HOST_CXX/HOST_AR — cargo-ndk exports plain CC/CXX/AR   pointing at NDK clang, which hijacked host-targeted units (sqlx-macros'   vendored OpenSSL for the host). - flake: add cmake (aws-lc-sys; was leaking in from the host) and SDK   platform 36 (tauri 2.11's :tauri-android gradle subproject).
  - Example app: - tauri android init scaffolding (gen/android); identifier ->   org.holochain.runtimeexample (Android forbids '-' in package segments). - mobile entry point + per-platform data dir: plugin registration moved   into setup() so mobile can use app_data_dir(); desktop keeps the   throwaway temp dir. - gradle: ndkVersion pinned and template keepDebugSymbols removed — the   unstripped debug .so made a 919 MB APK (stripped: 159 MB). - package.json + workspace wiring; build:holochain-runtime-example /   start:example-android root scripts.
  - Other apps: pin the same ndkVersion — AGP 8.6's default NDK is 26.1, which the old flake happened to ship; with only r28 installed they would silently package unstripped jniLibs.
- *(tauri-plugin-holochain)* Rebind a window's app in place (no recreate) by @zo-el
  - Direct mode binds an app to a webview at window creation, so a consuming app must destroy + rebuild the OS window to switch which app a window talks to -- on boot and on every dashboard<->network switch. That breaks WebDriver-based UI tooling (the session dies with "no such window"), drops window state, and flickers on desktop.
  - Add an in-place rebind so the OS window persists:
  - Rebind_window(label, Option<InstalledAppId>): update the app_request routing   (the window_apps map is already read per request), swap the window's signal   forwarder, and emit EVENT_REBOUND. None unbinds the window (app-less). - EVENT_REBOUND ("holochain://rebound"): payload is the new app id (or null); the   injected env updates __HC_TAURI_HOLOCHAIN__.INSTALLED_APP_ID in place and   dispatches a `holochain-rebound` DOM event, so @holochain/client can reconnect   to the new app without a reload. - Track per-window signal-forwarder task handles so a rebind (or the re-bind on   swap_runtime) aborts the previous forwarder rather than leaking the old app's   signal stream. - main_window_builder now takes Option<app_id>: None injects the IPC env (+ the   rebound listener) on an unbound window -- the dashboard -- to be bound later via   rebind_window, with no recreate.
  - Verified against unyt's direct-mode integration on the real Tauri/WebKitGTK app: the persistent window injects the env, renders, and rebinds without recreation.
- Controllable boot, hc-auth, restart-keeping-lair, seed export by @zippy
  - Phase 3 support for the unyt-on-ASR migration, on the in-process direct-mode plugin + runtime.
  - Controllable boot: init_deferred registers the plugin without booting;   HolochainPlugin::start / start_with_config(passphrase, config) boot the   conductor late (e.g. once a user-typed lair password is collected). The   runtime is held behind RwLock<Option<_>>; runtime() returns an owned clone;   new holochain://lair-ready event emitted before EVENT_READY. - Lair-first boot: spawn lair in-proc (at the same path holochain uses for   lair_root: None, so existing keystores are reused) and build the conductor   with .with_keystore(..), so the hc-auth flow can sign a challenge and inject   the auth material into the NetworkConfig before the network starts. - hc-auth (crates/runtime/src/hc_auth.rs): GET /now -> sign via lair -> PUT   /authenticate flow; HcAuthConfig / HcAuthStatus; runtime accessors   hc_auth_status / hc_auth_agent_key / hc_auth_raw_ed25519_b64url;   HolochainPluginConfig::with_hc_auth. - Restart-keeping-lair: stop_conductor_only + restart_with_hc_auth rebuild the   conductor on the still-running lair; plugin swap_runtime re-binds the   per-window signal forwarders. - Agent identity + backup: device_agent_key() (the device seed's ed25519 key, a   deterministic identity), export_agent_seed() (lair export_seed_by_tag + local   decrypt), with_pending_import_seed (import as the device seed at boot),   generate_agent_pub_key().
  - Adds deps: holochain_keystore, reqwest (rustls-tls), base64. Includes unit tests for the seed export/import round-trip (incl. a byte-exact ground-truth check) and device-key identity restore.
- Add Runtime::dump_network_stats() by @zippy
  - Public wrapper over AdminRequest::DumpNetworkStats returning the typed kitsune2_api::ApiTransportStats, so callers read transport_stats.backend directly rather than a stringified dump (what unyt's About dialog needs).
  - Adds kitsune2_api as an explicit dependency, pinned to 0.4.1 — the version holochain 0.6.1 already resolves — so it unifies to one copy. Includes a test asserting the backend field comes back populated.
- Serve direct-mode webviews over Tauri IPC with signal forwarding by @zippy
  - Main_window_builder now defaults to direct Tauri IPC instead of a loopback app websocket: it binds the window to the app, spawns a task that forwards the app's conductor signals to the window as holochain://signal events (msgpack-encoded, via Runtime::subscribe_to_app_signals), and injects __HC_TAURI_HOLOCHAIN__ so @holochain/client routes the App API through the app_request command. WindowOptions::use_app_websocket falls back to the legacy __HC_LAUNCHER_ENV__ wiring.
  - The injected guest-js gains injectHolochainTauriEnv, which sets the direct-mode env and bridges holochain://signal events to the transport's subscribeSignals callback using @tauri-apps/api; the shared zome-call signer is factored out and parameterized by plugin name. dist-js bundle rebuilt.
  - Adds tokio as a direct dependency for the signal-forwarding task.
- Subscribe to app signals from the runtime by @zippy
  - Add Runtime::subscribe_to_app_signals(app_id) -> broadcast::Receiver<Signal>, so a Tauri plugin can forward conductor signals to a webview without an app websocket. The conductor's signal broadcast is keyed by app, but its only public (non-test) accessor — get_signal_tx — is keyed by cell, so this first resolves one of the app's provisioned cells.
- Add in-process app_request IPC command to the plugin by @zippy
  - Add the Tauri-IPC counterpart to the runtime's handle_app_request: an app_request command that serves the full App API in-process, so a webview can use @holochain/client without a loopback app websocket.
  - HolochainPlugin gains a window-label -> app binding (bind_window) and   app_request_bytes(window_label, request): it decodes the msgpack App API   request with holochain's SerializedBytes codec (identical to the websocket   wire format), dispatches via Runtime::handle_app_request scoped to the app   the window is bound to, and returns the encoded response. The target app is   taken from the window, never the request, replacing the per-app websocket   auth token. - commands::app_request resolves the calling window's label and delegates.   Registered in the invoke handler, build.rs, and default permissions. - Error gains WindowNotBound and Serialization variants.
  - Integration test: a window bound to the forum app fetches AppInfo and makes a signed CallZome (get_all_posts -> empty) entirely over app_request_bytes, with no app interface attached; an unbound window is refused. Existing boot test refactored onto shared helpers.
  - Main_window_builder still injects the websocket env; switching it to inject the Tauri-direct env is a later step.
- Add in-process app-request dispatch to the runtime by @zippy
  - Add Runtime::handle_app_request, the App-API twin of the private req_admin_api: it routes an AppRequest for a given installed app through holochain's AppInterfaceApi and returns the AppResponse with no loopback websocket. This is the dispatch primitive the unified Tauri plugin will call to serve @holochain/client requests directly.
  - App-level failures come back as AppResponse::Error (not Err), matching the websocket interface, so the JS client's error handling is unchanged.
  - Tests dispatch AppRequest::AppInfo and a signed AppRequest::CallZome against the forum fixture, decoding the zome result to an empty Vec<Link>. The call-zome test uses the cell's real coordinator zome name ("posts", not the "forum" role name) and an encoded unit payload.
- Add in-process tauri-plugin-holochain + verified desktop example (Phase 4) by @zippy
  - A third Tauri plugin that runs a Holochain conductor in-process — no UniFFI, Kotlin, Android service, or cross-app AIDL. Built directly on holochain-conductor-runtime. The separated client + service plugins remain fully supported; this is the single-binary path.
- Switch to kitsune2 bootstrap & signal servers by @mattyg
- Add cloudflare to ice urls by @mattyg
- Pass runtime network settings when starting runtime, allow distributor of android-service-runtime app to configure by @mattyg
- Translations support by @mattyg
- Use dokka to generate docs site, publish to github pages in action by @mattyg
- Auto start service on system boot if it was previously running during system shutdown by @mattyg
- Configure ice urls by @mattyg
- Commands to lint tauri plugins by @mattyg
- Move setupApp logic down into runtime crate by @mattyg
- Split admin & app ipc binders, only same package can call admin binder, app binder only associated with a single app by @mattyg
- Restart entire app when 'reload' is clicked, ensuring the full plugin setup process runs again. This is necessary because only the plugin setup process can attach an initialization script to the webview that will run before the UI renders by @mattyg
- Cleanup implementation by @mattyg
- Not connected notice blurs & disables app functionality, center dialog on screen, supports light & dark mode by @mattyg
- Optionally call enableApp, after sucessfully installing app by @mattyg
- EnableApp command in consumer plugin by @mattyg
- Tokio multi-threaded runtime by @mattyg
- Example-client-app builds and runs -- failing to execute runtime commands by @mattyg
- Async non-blocking signZomeCall by @mattyg
- Async non-blocking ensureAppWebsocket by @mattyg
- Async non-blocking isAppInstalled by @mattyg
- Async non-blocking disableApp by @mattyg
- Async non-blocking enableApp by @mattyg
- Async non-blocking uninstallApp call by @mattyg
- Async non-blocking installApp call by @mattyg
- Async non-blocking listApps call by @mattyg
- IsReady call to check that conductor is available, avoid blocking main thread by launching conductor by @mattyg
- Remove get_admin_port -- we are no longer creating an admin websocket, but calling the conductor directly by @mattyg
- Implement holochain-conductor-runtime and ffi-bindings crates by @mattyg
- Add nix flakes by @mattyg
- Feature flag 'system_settings' to specify if android manifest should include app only in system settings home page, or in app grid by @mattyg
- Launch app from system settings homepage by @mattyg
- *(ui)* No apps installed message by @mattyg
- *(ui)* Remove prebundled apps by @mattyg
- Use renamed *-foreground-service -> *-service packages, app should only run on android mobile targets by @mattyg
- Codesigning by @mattyg
- App icon by @mattyg
- Add relay happ by @mattyg
- Reload installed apps list on nav by @mattyg
- Conductor status, start/stop conductor, list apps, enable/disable/uninstall apps, list prebundled happs, install prebundled happ by @mattyg

### Bug Fixes

- *(plugin)* Grant Linux camera and microphone only to the locked origin by @zippy in [#18](https://github.com/holochain/runtime-tauri/pull/18)
  - Answer each WebKitGTK user-media permission request against the origin the webview first loaded, and deny it for any other page, instead of granting media capture to every webview.
- \[**BREAKING**\] *(runtime)* Remove the legacy app websocket by @zippy in [#17](https://github.com/holochain/runtime-tauri/pull/17)
  - The legacy path attached an app interface with AllowedOrigins::Any and injected a reusable, non-expiring token into the page. No consumer uses it, so remove it rather than harden it (supersedes #19).
  - **Breaking Change**: WindowOptions::use_app_websocket, the injected
__HC_LAUNCHER_ENV__, Runtime::ensure_app_websocket, Runtime::setup_app and
AppAuth are removed. Use Runtime::install_app_if_missing instead of setup_app.
- *(plugin)* Lock windows to the origin they first load from by @zippy
  - Record each webview's origin from its first navigation or page load and refuse navigation anywhere else, except blob: URLs of that origin; window.open and target=_blank open nothing. main_window_builder applies this, and HolochainPlugin::lock_navigation applies it to windows an app builds itself. on_new_window needs Tauri 2.8, so the workspace floor moves to 2.8.0.
- *(plugin)* Request the restart in set_user_network_config and test the command end to end by @zippy in [#15](https://github.com/holochain/runtime-tauri/pull/15)
- \[**BREAKING**\] *(plugin)* Move the user network config commands under Tauri's ACL by @zippy
  - Get_, default_ and set_user_network_config were app commands, so any webview could call them. As plugin commands (plugin:hc|...) the capability system decides: hc:default grants the two reads, and set, which repoints bootstrap and relay and restarts the app, needs hc:allow-set-user-network-config. A missing UserNetworkConfigPath is now an error instead of a panic.
- *(security)* Ship a restrictive CSP in the generator and example app by @zippy in [#16](https://github.com/holochain/runtime-tauri/pull/16)
  - Replace csp: null with a policy allowing script from the app only, plus 'wasm-unsafe-eval' for libsodium, and connections only to Tauri IPC. The example turns withGlobalTauri off and invokes through @tauri-apps/api, and the generator test asserts the template and the example ship the same policy.
- *(android)* Migrate JNI_OnLoad to the jni 0.22 API by @zippy in [#11](https://github.com/holochain/runtime-tauri/pull/11)
  - Jni 0.22 splits JNIEnv into EnvUnowned (FFI) and Env (API), removes JavaVM::get_env() without a scope token, renames get_java_vm_pointer to get_raw, and makes JavaVM non-transparent. Take the raw VM pointer in JNI_OnLoad, upgrade it with JavaVM::from_raw, and borrow an Env through attach_current_thread (a no-op attachment on the already-attached loader thread). Class, method name and signature now use the jni_str!/jni_sig! macros since plain &str no longer satisfies AsRef<JNIStr>.
- *(plugin)* Fall back to the app name when the author is empty by @zippy
  - App_paths passes the author to app_dirs2, which rejects an empty one. A crate with no authors has an empty CARGO_PKG_AUTHORS, so an app generated without one panicked at launch with "Invalid app name or author".
- *(plugin)* Claim dev instance dirs where file locks are unsupported by @zippy
  - Std's File::try_lock is unsupported on Android, so app_paths failed with "try_lock() not supported" and Emergence aborted at launch. Only one copy of a mobile app runs at a time, so an unsupported lock now claims the slot.
- *(android)* Initialize ndk_context when the native library loads by @zippy
  - On Android the app aborted immediately on launch with "android context was not initialized". tao 0.35 (Tauri 2.11) no longer initializes ndk_context the way its ndk_glue used to, and several crates in the tree call ndk_context::android_context(): app_dirs2, iroh's DNS resolver (via hickory) and netdev. A panic crossing Tauri's JNI entry point aborts the process, and under panic = "abort" iroh's own catch_unwind around the DNS lookup cannot recover either.
  - The plugin now exports JNI_OnLoad, which runs at System.loadLibrary before any app code, and hands ndk_context the process's Application from ActivityThread.currentApplication(). Failures only log a warning.
  - Verified with Emergence on an x86_64 API 34 emulator: JNI_OnLoad is exported from libtauri_app_lib.so, the app launches, boots its conductor, creates a clone cell and reaches the profile form, with no panic in logcat.
- *(example)* Install and enable directly instead of setup_app by @zippy
  - Setup_app also attaches an app websocket interface, but the example's window reaches the conductor over Tauri IPC, so that port sat open and unused. The comment on the window builder still said it injected __HC_LAUNCHER_ENV__, which is the legacy websocket mode. The example is what app developers copy, so it now shows the IPC pattern: install and enable on first run only (an app a user disabled stays disabled), then open the IPC window.
- *(tauri-plugin-holochain)* Emit the setup-failed event from a deferred start by @zo-el
  - A deferred start recorded the cause but emitted nothing, so a frontend listening for the event saw no failure. Emitting from start_with_config covers both entry points, and the lock is released first so a listener can call back in.
- *(tauri-plugin-holochain)* Report a not-ready conductor from sign_zome_call by @zo-el
  - It panicked where its sibling sign_payload returns Error::NotReady, and a panic in a command handler does not reject the webview promise.
- *(tauri-plugin-holochain)* Report why the conductor failed to boot by @zo-el
  - Review follow-up: a failed boot burned the full wait and hid the real start() error.
  - The plugin held only `RwLock<Option<Runtime>>`, so a setup error existed solely as the transient `holochain://setup-failed` event. Anything not already listening when it fired lost the cause for good, which is why `try_runtime()` could only ever answer `NotReady`, indistinguishable from a boot still in flight. A host app whose conductor failed to come up had no way to learn why.
  - The runtime slot becomes a `BootState` of `NotStarted`, `Ready(Runtime)` or `Failed(String)` behind the same single lock, so the outcome is one piece of state and cannot disagree with itself. `start_with_config` records the cause of a failed boot and `try_runtime()` reports it as the new `Error::SetupFailed`; `NotReady` now means only that no boot has finished. `holochain://setup-failed` is emitted exactly as before, so existing consumers are unaffected, and the cause is recorded before the emit, so a listener that reacts to the event and then calls `try_runtime()` sees the same string.
  - `wait_for_ready` becomes a plain poll that panics on a reported setup failure, so a test fails on the actual error rather than sitting out `BOOT_TIMEOUT`. Against a conductor whose data root cannot be created, that is 0.2s reporting the lair error in place of 60.0s reporting only "conductor did not become ready".
  - `failed_boot_reports_its_cause_instead_of_timing_out` covers it, and fails both if the plugin stops recording the cause and if the waiter goes back to polling blindly.
- Surface lair's underlying error detail in RuntimeError::Lair by @zo-el
  - The Display impl dropped the wrapped OneErr's message, so every lair failure (locked keystore, missing key, any other cause) rendered as the identical bare string "Lair Error" with no way to tell them apart from the caller side.
- *(tauri-plugin-holochain)* Regenerate dist-js bundle + guard against staleness by @zo-el
  - B82f6ce changed EVENT_REBOUND to ReboundEvent { seq, app_id } and updated guest-js/index.ts, but the include_str!'d dist-js/holochain-env/index.min.js was not rebuilt — so the shipped listener still read e.payload (the whole object) as the app id and had no seq gate (#3's fix wasn't in effect at runtime). Rebuilt via `npm run build`, and added an integration test asserting the bundle reads the structured payload fields, since no runtime test exercises the injected JS.
- *(tauri-plugin-holochain)* Harden rebind_window per review — rollback, destroy cleanup, ordered rebound by @zo-el
  - Addresses zippy's review on PR #141: - #1 spawn-before-bind: rebind_window spawns the new forwarder before flipping   routing, so a failed rebind keeps the prior binding (was: routing flipped   before the fallible spawn, with no rollback). - #2 window-destroy cleanup: an on_event(RunEvent::WindowEvent::Destroyed)   handler prunes the window maps and aborts the forwarder (was: never pruned). - #3 ordered rebound: EVENT_REBOUND carries a monotonic seq and the injected env   applies only newer ones, so an out-of-order rebound can't strand the UI.
- Update the Develpment readme for correct publication order when bumping hc version by @zippy
- Invalid bootstrap url by @mattyg
- Network config json schema by @mattyg
- Update .happ text fixture to 0.5 by @mattyg
- Agent keys type cast by @mattyg
- Set default provider in rustls by @mattyg
- Copy required native library lib++_shared.so into jniLibs in build script by @mattyg
- Failing test by @mattyg
- Doc building by @mattyg
- Build tauri plugins for android arch so the tauri-android project gets generated by @mattyg
- Ci environment with signing keys by @mattyg
- Sign builds by @mattyg
- Build tauri plugin scripts by @mattyg
- Ci syntax by @mattyg
- Building by @mattyg
- Build in nix by @mattyg
- Ensure every gradle project expects the same android tool versions, which are the same as those provided in nix shell by @mattyg
- Test assertion by @mattyg
- Test by @mattyg
- Avoid crash when navigating to 'my apps' page by @mattyg
- Config parceler and add test by @mattyg
- Rm deprecated jvm config by @mattyg
- Build ffi crates for release by @mattyg
- Export service from dep library so intent can access it by @mattyg
- Wrong output path for ffi lib builds by @mattyg
- Publish holochain-service-client to mavenLocal before building holochain-service by @mattyg
- Lint fix before building holochain-service-client, to ensure that generated kotlin bindings do not voilate linting rules and thus fail the build by @mattyg
- Non-existing command by @mattyg
- Publish client to mavenLocal so it can be used by service lib by @mattyg
- Clippy by @mattyg
- Compile derps by @mattyg
- Comma by @mattyg
- Make InstallAppPayloadFfi require installedAppId by @mattyg
- Delete duplicate types-ffi lib -- we will already have it from the holochain-service-client by @mattyg
- Build crate before cargo ndk build works -- not sure why by @mattyg
- Build client before running both apps by @mattyg
- Fmt + clippy by @mattyg
- SetupApp working again by @mattyg
- One test thread by @mattyg
- Run static & tests in separate ci by @mattyg
- Avoid inconstent app crashes on restart due to 'logger initialized twice' error by @mattyg
- Merge clobber by @mattyg
- Hacky workaround aiming to ensure that __HC_LAUNCHER_ENV__ is defined before the UI is rendered by @mattyg
- Do not create tokio runtime in test context, because tests create a runtime themselves by @mattyg
- Load running status upon app launch by @mattyg
- Ci syntax by @mattyg
- Switch to uniffi-rs fork temporarily by @mattyg
- EnsureAppWebsocket response type by @mattyg
- Tauri InvokeArgs erroring when passing an data class as a field by @mattyg
- Swap order of parameters by @mattyg
- Consumer plugin args working with changes to client by @mattyg
- Json serialization support for sealed classes (i.e. converted rust enums) by @mattyg
- Android-service-runtime crashing when calling an ffi function in release builds by @mattyg
- Separate cache for each job by @mattyg
- Need all targets in rust-toolchain.toml by @mattyg
- Toolchain version and ocmopnents by @mattyg
- Rust-toolchain.toml by @mattyg
- Specify rust version in rust-toolchain.toml by @mattyg
- Build dependency crate and bindings before running tests by @mattyg
- Linebreak by @mattyg
- Path by @mattyg
- Path by @mattyg
- Path by @mattyg
- Path by @mattyg
- Create gradle.properties as it is not included in repo by @mattyg
- Api level 28, so we can run on x86_64 on ubuntu by @mattyg
- Run on macos for armv8 support by @mattyg
- Avd cache key by @mattyg
- Missing system image by @mattyg
- Cache ci setup by @mattyg
- Bump jdk, specify ndk by @mattyg
- Enable kvm & specify arch in matrix by @mattyg
- Indent by @mattyg
- Version by @mattyg
- Avoid importing holochain in types-ffi crate, include built types_ffi.so lib in kotlin client by @mattyg
- Fn name changed, clippy by @mattyg
- Input type by @mattyg
- Admin port no longer used by @mattyg
- Notification icon by @mattyg
- Renamed commands by @mattyg
- Default service package name & bump holochain-service-client by @mattyg
- Move error types from holochain-conductor-runtime-types-ffi to holochain-conductor-runtime-ffi -- uniffi-rs does not support error types in external crates by @mattyg
- Remove unnecessary unused fns and types exposing plugin calls on rust side of tauri plugins by @mattyg
- Ffi-types crate building again by @mattyg
- Armv7 not supported by emulator runner by @mattyg
- Add logs to ffi calls, clippy fix by @mattyg
- Only run tests in packages by @mattyg
- Clearer errors, avoid ambiguous error casting by @mattyg
- Rename app_websocket_auth -> ensure_app_websocket by @mattyg
- Uniffi config for generating kotlin bindings by @mattyg
- Compatability with changes to holochain_runtime by @mattyg
- Wrong holonix version by @mattyg
- Error message in toast by @mattyg

### Miscellaneous Tasks

- Remove todo steps from github pr template -- changelog is generated automatically for releases from commit messages, and ci runs on prs automatically by @mattyg in [#29](https://github.com/holochain/runtime-tauri/pull/29)
- *(nix)* Update rust-overlay by @jost-s in [#5](https://github.com/holochain/runtime-tauri/pull/5)
- Update the AI_POLICY.md with shared content in [#4](https://github.com/holochain/runtime-tauri/pull/4)
- Update the CONTRIBUTING.md with shared content
- Remove material inherited from android-service-runtime by @zippy
  - This repository keeps android-service-runtime's history but none of its releases or plans:
  - Reset holochain-conductor-runtime and tauri-plugin-hc to 0.1.0; neither has   been published, and 0.3.0 was the android-service-runtime release line. - Drop the 0.1.0-0.3.0 CHANGELOG sections and the Unreleased entries that only   described the split from that repository. - Remove the Holochain 0.6/0.7 update plans and the direct app/admin call plans,   all written for the service/client stack and now complete or moot. - Replace the README's section on the Android service model with a line on   where this repository came from, and drop wording that only contrasted the   plugin with it.
- Rename the repository to runtime-tauri by @zippy
- Drop the dokka docs step from the PR template by @zippy
  - `pnpm run build:doc` built the Kotlin library docs and no longer exists.
- Rework the root build scripts for the runtime workspace by @zippy
  - Package.json drops every Android-service, Kotlin and UniFFI script and gains iOS entry points. The Makefile's integration-test and lint targets now cover the two remaining crates via --workspace. pnpm workspace tracks the plugin and the example app.
- Scope the workspace to the cross-platform runtime by @zippy
  - Members are now the runtime, the in-process plugin and the example app. Drops the workspace dependencies left without a consumer once the Android FFI stack moved out: uniffi, android_logger, holochain_conductor_api, holochain_serialized_bytes and bytes.
  - Also drops the [patch.crates-io] block the ios branch carried. It pointed at a sibling ../holochain checkout on branch ios-inproc-keystore, which does not exist (that checkout is on develop at 0.8.0-dev.0), and its own commit message flagged it as MUST NOT merge to main. See the iOS note in the README.
- Bump versions for 0.3.0 release by @zippy
  - Rust crates 0.2.2 -> 0.3.0 for the holochain 0.7 migration and the breaking RuntimeNetworkConfig / SetupFailed changes; the android-service-runtime app 0.1.5 -> 0.2.0. The Kotlin client and service libraries are left on their current versions.
- Released @holochain/client 0.21.0 + gate desktop plugin suite in CI by @zippy
  - JS client: all seven package.jsons move @holochain/client from the local file:../holochain-client-js checkout (at 0.21.0-rc.1) to the released ^0.21.0 on npm; pnpm workspace lockfile regenerated. pnpm 11 build-script approvals (bufferutil, electron, es5-ext, esbuild) recorded in pnpm-workspace.yaml so CI installs don't fail on ERR_PNPM_IGNORED_BUILDS. The rebuilt dist-js/holochain-env/index.min.js is byte-identical (the client import is type-only), so the shipped bundle was already current — verified by the staleness-guard integration test.
- *(deps)* Bump holochain to released 0.7.0 by @zippy
  - Workspace pins move off 0.7.0-dev.28 to the 0.7.0 release: holochain, holochain_conductor_api, holochain_types, holochain_keystore -> 0.7.0, kitsune2_api -> 0.5.0, hdk 0.7.0 / hdi 0.8.0 in the example app. The lock unifies the kitsune2 family on 0.5.0 (drops the dev-era iroh 1.0.0-rc.0 pin) and the unused wasmer fix-x86 [patch.crates-io] is removed (0.7 is on wasmer 7.1). flake: holonix main-0.6 -> main-0.7 (dev shell ships holochain 0.7.0); nodejs_20 -> nodejs_22 (20 is EOL, refused by nixpkgs).
  - API changes absorbed: - InstallAppPayload.restore_from_dht: set false everywhere; not exposed   over FFI yet. - AppStatus::{AwaitingRestore, Unrecoverable}: added to AppStatusFfi   (Unrecoverable carries CellIdFfi + rendered reason string). - NetworkConfig lost signal_url/webrtc_config (tx5/WebRTC replaced by   iroh): signal_url and ice_urls removed from RuntimeNetworkConfig,   RuntimeNetworkConfigFfi, the FFI mappers, and the ASR app (BREAKING for   the FFI/Kotlin surface). - RuntimeError boxes ConductorError/AdminResponse (clippy result_large_err   under rust 1.95). - test_install_with_foreign_key_fails_genesis: 0.7 rejects a non-keystore   agent key up front with AgentKeyNotInKeystore instead of failing genesis. - forum.happ fixtures repacked from hdk 0.7.0 zomes: coordinator signal   emission ported to the Action/ActionData restructure; the scaffolded   permissive per-op validation boilerplate replaced by an explicit   accept-all validate (the fixture exercises the runtime, not validation).
  - Make static passes; runtime 29/29, runtime-ffi 12/12, tauri-plugin integration 6/6 against 0.7.0.
- *(deps)* Bump holochain to 0.6.3 by @zippy
  - Workspace pins (holochain, holochain_conductor_api, holochain_types) and the runtime crate's holochain_keystore -> 0.6.3; Cargo.lock moves the holochain crate family accordingly. nix flake update advances holonix (main-0.6) so the dev shell ships holochain 0.6.3.
  - Also fix holochain-runtime-example for the main_window_builder signature change from the window-rebind PR (app id is now Option<String>).
- *(deps)* Update Cargo.lock to holochain 0.6.2-rc.0 by @zo-el
  - The 058073e Cargo.toml bump didn't update the lock, leaving it stale at 0.6.1 — inconsistent with the 0.6.2-rc.0 manifest (cargo build --locked would fail). Re-resolves the holochain crates to 0.6.2-rc.0.
- *(deps)* Bump holochain to 0.6.2-rc.0 by @zo-el
  - Workspace pins (holochain, holochain_conductor_api, holochain_types) -> 0.6.2-rc.0, holochain_serialized_bytes -> 0.0.57, and the runtime crate's holochain_keystore -> 0.6.2-rc.0. HC-795 adds init_properties to RoleSettings::Provisioned; the FFI RoleSettingsFfi -> RoleSettings conversion sets it to None (the FFI surface does not carry it yet).
- Bump ASR app version to 0.1.3 for release by @zippy
- More version-numbers by @zippy
- Bump versions for 0.2.2 release by @zippy
- Changelog by @mattyg
- Fmt by @mattyg
- Avoid glob version by @mattyg
- Update versions & libraries by @zippy
- WIP bump version numbers by @zippy
- Update flake by @zippy
- Changelog by @mattyg
- Fmt + clippy by @mattyg
- Changelog by @mattyg
- Fmt + clippy by @mattyg
- Clarity by @mattyg
- Typo by @mattyg
- Clarity by @mattyg
- Typo by @mattyg
- Typo by @mattyg
- Typos by @mattyg
- Clippy by @mattyg
- Changelog by @mattyg
- Call npm command from ci by @mattyg
- Lint + fmt by @mattyg
- Typo by @mattyg
- Changelog by @mattyg
- Flake lock by @mattyg
- Rm unncessary deps by @mattyg
- Workflow name by @mattyg
- Cargo lock by @mattyg
- Changelog by @mattyg
- Changelog by @mattyg
- Rm redundant package def, link to new clearer ticket describing why crate is forked by @mattyg
- Cleanup cargo package info by @mattyg
- Changelog by @mattyg
- Fmt + clippy by @mattyg
- Changelog by @mattyg
- Fmt + clippy by @mattyg
- Full word by @mattyg
- Changelog by @mattyg
- Changelog by @mattyg
- Fmt + clippy by @mattyg
- Changelog by @mattyg
- Changelog by @mattyg
- Changelog by @mattyg
- Fmt + clippy by @mattyg
- Changelog by @mattyg
- Don't overwrite changelog by @mattyg
- Changelog by @mattyg
- Reorder package scripts for consistency by @mattyg
- Rename crate dirs from holochain-conductor-runtime* to runtime* to reduce noise by @mattyg
- Rename tauri-plugin-holochain-service-consumer to tauri-plugin-holochain-service-client for consistency and change directory names to tauri-plugin-service and tauri-plugin-client to reduce verbosity by @mattyg
- Rename tauri plugin packages to resolve remainint lint issues by @mattyg
- Fix nearly all remaining lint issues in client, service, tauri plugins by @mattyg
- Rename org.holochain.androidserviceclient.client -> org.holochain.androidserviceclient.service to avoid ktlint complaint about package names with underscores by @mattyg
- Rename org.holochain.androidserviceclient.holochain_service_client -> org.holochain.androidserviceclient.client to avoid ktlint complaint about package names with underscores by @mattyg
- Fix lint issues with tauri plugins by @mattyg
- Rm and gitignore ide artifact *.iml by @mattyg
- Move tests from tauri plugin to kotlin lib by @mattyg
- Changelog by @mattyg
- Clean imports, test naming casing by @mattyg
- Changelog by @mattyg
- Clippy by @mattyg
- Fmt by @mattyg
- Log when app already installed by @mattyg
- Fmt + clippy by @mattyg
- Explanatory comments by @mattyg
- Changelog by @mattyg
- Changelog by @mattyg
- Rename make command by @mattyg
- Changelog by @mattyg
- Typo by @mattyg
- Changelog by @mattyg
- Readme mistakes by @mattyg
- Clippy by @mattyg
- Changelog by @mattyg
- Cargo lock by @mattyg
- Explicitly call function on window. by @mattyg
- Cleanup by @mattyg
- Rename for clarity by @mattyg
- DRY exception handling by @mattyg
- Schemas by @mattyg
- Simplify types by @mattyg
- Fmt + clippy by @mattyg
- Changelog by @mattyg
- Changelog by @mattyg
- Schemas by @mattyg
- Fmt + clippy by @mattyg
- Unused gitignore by @mattyg
- Npm scripts to start apps also build prereq tauri plugins by @mattyg
- Tauri autogens by @mattyg
- Fmt + clippy by @mattyg
- Rm unused function by @mattyg
- Accidental commit by @mattyg
- Changelog by @mattyg
- Rename holochain env build artifact, remove from repo by @mattyg
- Lint by @mattyg
- Changelog by @mattyg
- Spcaing by @mattyg
- Changelog by @mattyg
- Add scripts to root package.json by @mattyg
- Move and rename example/service-consumer-app to apps/example-client-app by @mattyg
- Delete example service app, we have the android-service-runtime app example by @mattyg
- Rename package uniffi-bindgen -> uniffi-bindgen-cli for clarity by @mattyg
- Changelog by @mattyg
- Use IO dispatcher for runtime calls by @mattyg
- Changelog by @mattyg
- Define outside constructor by @mattyg
- Typo by @mattyg
- Logs in changelog by @mattyg
- Changelog by @mattyg
- Changelog by @mattyg
- Move uniffi-bindgen cli into its own crate to avoid rebuilding by @mattyg
- Changelog by @mattyg
- Clippy by @mattyg
- Explicitly specify service class name and app name in tauri plugins by @mattyg
- Only suspend call inside runBlocking by @mattyg
- More logging by @mattyg
- Rename library from holochain-service-types -> holochain-service-client by @mattyg
- Easier package.json scripts for building plugins and running app by @mattyg
- Fmt by @mattyg
- Readme typo by @mattyg
- Add PR template by @mattyg
- Changelog by @mattyg
- Rebuild ffi crate and regenerate bindings by @mattyg
- Fmt + clippy by @mattyg
- Rm cspell words by @mattyg
- Move example apps to repo root -- they still haven't been updated to work properly by @mattyg
- Changelog by @mattyg
- Changelog by @mattyg
- Fmt by @mattyg
- Rename package by @mattyg
- Rename package by @mattyg
- Artifacts by @mattyg
- Artifacts by @mattyg
- Log by @mattyg
- Rename app by @mattyg

### Build System

- Select npm workspaces by name, not path by @zippy
  - Npm treats a path given to -w as a directory filter that also matches every workspace nested under it, so `-w apps/holochain-runtime-example` ran build:android in apps/holochain-runtime-example/ui as well, which has no such script. The APK had already been built when it failed. Workspace names match exactly.
- Replace the Makefile with npm scripts and build the example UI before clippy by @zippy
  - Compiles the example app, and Tauri's generate_context! panics when frontendDist (ui/dist) does not exist. The Makefile's lint target ran clippy before anything built the UI, so the Test workflow failed on a fresh checkout.
  - Every Makefile target already had an npm twin, so the Makefile goes and `npm run ci` (fmt:check, lint, test) is what CI runs. `lint` now builds the example UI first. The uncommitted-changes guard moves to its own workflow step.
- Drop dependency debug info in dev builds by @zippy
  - With full debug info for dependencies, Emergence's Android debug library was 1,313 MB, of which 1,180 MB was DWARF (.debug_info 525 MB, .debug_loc 300 MB, .debug_str 192 MB) against 68 MB of code, and the 1.3 GB APK failed to install on an emulator with INSTALL_FAILED_INSUFFICIENT_STORAGE.
  - [profile.dev.package."*"] now also sets debug = false. The workspace's own crates keep their debug info, and dependency symbols are not stripped, so backtraces still name functions. The plugin README's recommended block for apps includes it.
- *(nix)* Export tauriDev and tauriAndroidDev shells for apps by @zippy
  - Apps built on tauri-plugin-hc need the same toolchain this repo tests against: Rust with the wasm32 and mobile targets, the holonix tools, nix-provided webkit/GTK (a host webkit mixed with the nix compiler fails to link against glibc), and for Android the SDK/NDK, JDK and cargo-ndk. The single default shell is split so apps can compose the pieces, the way apps once composed p2p-shipyard's holochainTauriDev / holochainTauriAndroidDev:
  - DevShells.default = pkgs.mkShell {     inputsFrom = [ inputs'.tauri-runtime.devShells.tauriDev ];     packages = [ pkgs.nodejs_24 ];   };
  - TauriDev covers desktop apps and zome builds; tauriAndroidDev adds Android. Neither provides Node or sets PS1, so apps pick their own. This repo's default shell is both plus nodejs_22 and hc-scaffold, unchanged in content.
- Optimize dependencies in dev and test builds by @zippy
  - Unoptimized, the conductor, wasmer/cranelift and lair are an order of magnitude slower. Timed with holochain-conductor-runtime installing Emergence's happ on a single-agent local network:
  - Phase                        debug    release  debug + deps opt 3   boot (lair + conductor)     19.65s     1.74s     1.72s   install (genesis)           18.22s     1.22s     1.40s   first call on a new clone   29.57s     2.48s     2.89s
  - The first call on a new cell compiles the zomes and runs init(); later calls took 10 ms in every profile. With [profile.dev.package."*"] opt-level = 3 in the workspace root, workspace crates stay unoptimized, so editing them still rebuilds quickly. The test suites speed up too: runtime 163s -> 39s, plugin integration 44s -> 5s.
  - Profiles only apply in a workspace root, so the plugin README tells app developers to set the same thing, and DEVELOPMENT.md records why.
- Move JavaScript tooling from pnpm to npm workspaces by @zippy
  - The rest of the Holochain app ecosystem is on npm or Yarn classic. pnpm came along from android-service-runtime rather than being chosen, and the repo was only half on it: the example UI was installed with npm --prefix outside the pnpm workspace, with its package-lock.json gitignored, so CI resolved its dependencies fresh on every run.
  - The plugin, the example app and its UI are now npm workspaces covered by one committed package-lock.json. CI runs npm ci. The flake no longer provides pnpm. The generated Android Gradle task and iOS Xcode build phase invoke npm run tauri -- ... instead of pnpm tauri ..., which they had baked in at tauri init time.
  - The plugin's bundled runtime dependencies are locked at the versions the committed bundle was built from (@tauri-apps/api 2.5.0, @msgpack/msgpack 3.1.2); npm would otherwise have resolved newer ones inside the same ranges and changed the injected code. The rebuilt bundle differs from the previous one only in minifier identifier names and redundant parentheses.
- Drop the webkitgtk 2.42 pin; fix EGL vendor discovery in the dev shell by @zippy
  - The blank-webview abort ("Could not create default EGL display: EGL_BAD_PARAMETER") was never a webkitgtk regression: nixpkgs' libglvnd only searches /run/opengl-driver — a NixOS-only path — for EGL vendor drivers, so on other distros nix-built webkit finds no driver at all. webkitgtk 2.42 tolerated that; >= 2.44 requires EGL in the web process (DMA-BUF renderer + Skia) and 2.52 aborts.
  - Instead of pinning old webkit, the dev shell now exports an ordered __EGL_VENDOR_LIBRARY_DIRS on non-NixOS hosts: the host's native glvnd dirs first (host binaries launched from the shell behave exactly as outside it), nixpkgs Mesa last, which nix-linked binaries fall back to after failing to dlopen the host driver (llvmpipe on NVIDIA; drives AMD/Intel directly). Verified end-to-end on webkitgtk 2.52.5: the example app renders in the dev shell, and a system-webkit (2.50.2) build outside nix is unaffected.
  - This removes the webkitnixpkgs input, the WEBKIT_DISABLE_COMPOSITING_MODE machinery (a no-op since webkitgtk 2.46, including the stale workaround in example-client-app's main.rs), the GIO_MODULE_DIR override (it masks the default GIO module search for every GLib app run from the shell; the additive GIO_EXTRA_MODULES stays), and the inert wrapGAppsHook3 (its wrapping never runs at shell entry); XDG_DATA_DIRS now uses the hook-populated GSETTINGS_SCHEMAS_PATH instead of hand-built paths.
- Allow opting into GPU compositing in the dev shell by @zippy
  - The dev shell forces WEBKIT_DISABLE_COMPOSITING_MODE=1 so `tauri dev` renders on finicky GPUs/drivers. Gate that behind ENABLE_WEBKIT_COMPOSITING: set it before `nix develop` to keep hardware-accelerated compositing instead.
- Pin webkitgtk to 2.42.5 in the dev shell by @zippy
  - Holonix's nixpkgs ships webkitgtk 2.52.x, which aborts with "Could not create default EGL display: EGL_BAD_PARAMETER" and renders a blank Tauri webview on non-NixOS GPUs during `tauri dev`. Source the GTK/webkit runtime stack from a pinned nixpkgs (webkitgtk 2.42.5) plus wrapGAppsHook and the GIO/XDG/WEBKIT_DISABLE_COMPOSITING_MODE env.
  - Dev-shell only; production bundles link the system/CI webkit.
- Migrate to Holochain 0.6.1 by @zippy
  - Phase 1 of the 0.6 + unified-plugin plan: bring the Holochain version-advancing work over from the volla-cloud-services fork, excluding all Volla-specific branding, Nexus registry, and CI changes. Targets the released 0.6.1 directly (folding in the plan's Phase 2) since 0.6.1 is out and holonix main-0.6 ships it.
  - Dependencies / toolchain: - holochain/conductor_api/types 0.5.6 -> 0.6.1, lair_keystore_api 0.6.2 ->   0.6.3; example app hdi 0.6.5 -> 0.7.0, hdk 0.5.5 -> 0.6.0 - flake holonix main-0.5 -> main-0.6 (holochain 0.6.1 binary, hc-scaffold   0.601.1); rust-toolchain channel 1.81.0 -> 1.88.0 - runtime: move uuid to [dependencies] (used by import_key_seed);   runtime-types-ffi: add bytes - pin RustCrypto RC family in Cargo.lock (pkcs8 0.11.0-rc.11,   ed25519 3.0.0-rc.4, signature 3.0.0-rc.10) -- the final releases are   incompatible with ed25519-dalek 3.0.0-pre.1 still required via iroh
  - API migration (runtime, runtime-ffi, runtime-types-ffi + regenerated Kotlin/AIDL): - AppInfoStatus -> AppStatus (Running -> Enabled, tuple Disabled; drop   Paused/PausedAppReason and DeletingAgentKey) - AdminResponse::AppEnabled(app); AppBundleSource::Bytes(_.into()); drop   allow_throwaway_random_agent_key; add danger_bind_addr; drop   RoleSettingsFfi::UseExisting - example zomes: GetLinksInputBuilder -> LinkQuery + GetStrategy,   delete_link with GetOptions; manifests to the 0.6 format
  - New features carried over: - import_key_seed (Lair seed import) + RuntimeError::InvalidArguments - optional agent_key on install; relay_url (iroh) plumbed through config
  - Defaults use Holochain public infrastructure (not *.volla.tech): bootstrap/signal dev-test-bootstrap2.holochain.org, iroh relay use1-1.relay.n0.iroh-canary.iroh.link.
- Own the nix dev shell, drop darksoil tauri-plugin-holochain flake by @zippy
  - Phase 0 of the 0.6 + unified-plugin plan. Replace the darksoil-studio tauri-plugin-holochain dev-shell input (its only use in this repo) with a dev shell composed from first principles in flake.nix:
  - Holochain toolchain (holochain, hc, hc-scaffold, lair-keystore, ...) from   holonix packages - Rust from rust-overlay via rust-toolchain.toml (single source of truth for   channel/components/targets) - Android SDK/NDK (compileSdk/buildTools 34, NDK 26.1.10909125) via nixpkgs   androidenv, exporting ANDROID_HOME/ANDROID_NDK(_ROOT/_HOME)/NDK_HOME - cargo-ndk, nodejs, pnpm, jdk17, binaryen, and Tauri v2 desktop libs
  - Extend rust-toolchain.toml with the Android (arm64/armv7/x86/x86_64) and wasm32 targets the build scripts and hApp builds require.
  - Flake.lock now references only holonix (+ followed nixpkgs/flake-parts/ rust-overlay); the darksoil and webkitnixpkgs nodes are gone. Verified nix develop yields the full toolchain and cargo-ndk cross-compiles a cdylib to valid arm64-v8a and x86_64 Android .so files.
- Bump msgpack, holochain client by @mattyg
- Bump version by @mattyg
- Bump crates by @mattyg
- Bump types-ffi crate by @mattyg
- Re-add pnpm by @mattyg
- Bump cargo locks by @mattyg
- Bump holochain client dep by @mattyg
- Upgrade to holochain 0.5 by @mattyg
- Revert to p2p-shipyard's nix flake by @mattyg
- Bump versions by @mattyg
- Bump version to test publish by @mattyg
- Npm scripts to build tauri plugins for a single target, build only for single target in CI by @mattyg
- Refactor nix flake to use androidenv by @mattyg
- Additional deps in nix flake by @mattyg
- Matching ndk version by @mattyg
- Bump tauri plugin deps by @mattyg
- Matching ndk version by @mattyg
- Use package name alias for uniffi fork by @mattyg
- Publish fork of uniffi-rs, instead of using git dependency by @mattyg
- Fix android build tool versions by @mattyg
- Bump android deps again by @mattyg
- Bump android tools again by @mattyg
- Bump to latest android biuld tools by @mattyg
- Add example-client-app/ui to workspace by @mattyg
- Move example-client-app into pnpm workspace for now by @mattyg
- Add msgpack dep by @mattyg
- Add msgpack dep by @mattyg
- Add lint npm command and run in ci by @mattyg
- More package json commands by @mattyg
- Cleanup package json scripts by @mattyg
- Prevent builds that fail linting, exclude generated kotlin bindings from lint check by @mattyg
- Add ktlint gradle plugin to ensure we are using the same formatter & linter that uniffi runs internally by @mattyg
- Increase memory available to jvm, include gradle.properties.ci template specifically for ci by @mattyg
- Switch from ktFmt to ktlint, as that is the linter run by uniffi after generating bindings by @mattyg
- Do not build rust libs and bindings for armv7, which is currently not supported by holochain by @mattyg
- Publish latest client by @mattyg
- Remove unused holonix binaries by @mattyg
- Add pnpm to flake by @mattyg
- Implement new nix flake for android + holochain development not relying on p2p-shipyard by @mattyg
- Bump tauri deps by @mattyg
- Ensure gradle dir is included in git repo for every project by @mattyg
- Make androidDev the default & only nix shell by @mattyg
- Ensure all gradle projects on same version of gradle by @mattyg
- Publish client library and bump versions to use by @mattyg
- Consistent ordering of maven repo sources on all gradle projects by @mattyg
- Command to publish to local maven repo, use deps from local maven repo *before* checking maven central repo. Allows us to change kotlin library and review without needing to publish to maven central by @mattyg
- Rm no longer needed imports and uniffi features by @mattyg
- Bump holochain deps, fix warning by @mattyg
- Rename packages to be under group id org.holochain.androidserviceruntime, rename app to 'Holochain Service Runtime' by @mattyg
- Publish holochain-service-types package to Maven Central Reposistory, via gradle command. Basic docs for setting up publish environment and example gradle.properties which will now contain secrets by @mattyg
- Bump android gradle plugin version and ignore warnings related to D8 not supporting certain java classes by @mattyg
- Use holochain-service-types lib as .aab file in tauri plugins -- importing properly by @mattyg
- Add npm packages to pnpm workspace, bump holochain client, remove build artifacts from repo by @mattyg
- Integrate fixes to tauri-plugin-holochain-service by @mattyg
- Bump tauri-plugin-holochain-service by @mattyg

### CI

- Add release workflows by @mattyg
- Cancel superseded runs on the same ref by @zippy
- Merge workflows into ci.yml with a ci_pass gate job by @zippy
  - The org-wide rulesets in holochain/hc-github-config require a single status check named ci_pass on main and the release branches. Jobs can only depend on jobs in the same workflow, so build.yml and test.yml are merged into ci.yml and a ci_pass job gates on all of them.
- Remove tool-cache to free disk space by @mattyg
- Re-add android to job by @mattyg
- Free disk space by @mattyg
- Free additional disk space by @mattyg
- Run kotlin test setup in nix by @mattyg
- Run static & rust tests in nix by @mattyg
- Build android-service-runtime with both feature options by @mattyg
- Build tauri plugins and apps by @mattyg
- Github action for client releases by @mattyg
- Fix syntax by @mattyg
- Only run jobs on prs to main and main-* by @mattyg
- Test holochain-service lib in ci by @mattyg
- Free disk space on runner by @mattyg
- Don't delete ndk to free space by @mattyg
- Free disk space by @mattyg
- Run ci on PRs that are not merging into main by @mattyg
- Run android tests on emulator in ci by @mattyg
- Build tests on host, then run in emulator by @mattyg
- Only run on one android api level by @mattyg
- Run tests on android emulator by @mattyg
- Run tests on ubuntu by @mattyg
- Static checks by @mattyg

### Testing

- *(plugin)* Cover set_ and default_user_network_config and the default permission set by @zippy
- Install a test hApp built from in-repo zomes by @zippy in [#13](https://github.com/holochain/runtime-tauri/pull/13)
  - The runtime and plugin test suites no longer install the prebuilt `forum.happ`. A new unpublished `test-happ` crate compiles a minimal integrity/coordinator zome pair to wasm in its build script, against the hdk/hdi of the holochain release the workspace depends on, and packs it into a `.happ` at test time. A holochain bump now only needs those versions bumped, not a fixture regenerated with `hc-scaffold`.
  - `forum.happ` moves to the example app, its only remaining user, so it no longer ships inside the published `holochain-conductor-runtime` crate.
- *(holochain-conductor-runtime)* Cover GenesisFailed on install under a non-local agent key by @zo-el
  - Installing an app under an agent key that lives in a different lair (so the local keystore can't sign for it) fails genesis with SourceChainError "Query returned no rows" — pinning the contract behind a consumer's stale-persisted-key symptom.
- *(tauri-plugin-holochain)* Cover window rebind, fix wait_for_ready gate by @zo-el
  - Rebind_window_reroutes_app_request_in_place binds a window to one app, rebinds to a second (app_request routing follows the binding), then unbinds (asserting WindowNotBound).
  - Wait_for_ready gated on holochain() — Ok as soon as the plugin is registered, before the conductor boots (init_deferred relies on that) — so it never waited and the two existing integration tests passed only by timing. Gate on try_runtime() instead, and correct the holochain() doc comment to match.
- Exercise direct Tauri IPC end-to-end in the desktop example by @zippy
  - Update the holochain-runtime-example UI to demonstrate (and verify) the plugin's now-default direct mode: report the injected __HC_TAURI_HOLOCHAIN__ env, connect @holochain/client over Tauri IPC, make a read (get_all_posts) and a write (create_post) zome call, and confirm the create_post signal arrives via client.on("signal"). Adds create_post + signal panels to index.html.
  - Point the example UI at the sibling ../holochain-client-js (file: dep), which carries the unreleased Tauri transport; bump to the published version once it ships.
  - Verified by running the app (DISPLAY, dev shell): env=direct, connect + appInfo, get_all_posts, create_post, and a type=app zome=posts signal all succeed over IPC with no app websocket attached.
- Add top-level runner for the tauri plugin and desktop example by @zippy
  - Add `test:tauri-plugin-holochain` (the plugin integration test that boots a conductor in a Tauri app), `test:example` (build the example UI + `cargo build` it), and a combined `test:tauri-plugin`, wired into the root `test` script. Also add `build:example` and `start:example` helpers.
- Verify Holochain 0.6.1 migration and new features (Phase 3) by @zippy
  - Add runtime integration tests for the features carried over in the 0.6.1 migration, and record the Phase 4 consumer-app findings in the plan.
  - Test_import_key_seed: importing the same seed into a *fresh* keystore   reproduces the same AgentPubKey (seed-based identity recovery), and a   different seed yields a different key - test_install_app_with_agent_key: installing with an explicit agent_key   yields an app whose agent_pub_key matches the imported key - test_new_runtime: assert relay_url is honored in the conductor network   config (alongside the existing bootstrap/signal assertions)
- Tests for persisting authorization working by @mattyg
- Binding to HolochainService basic tests and todos by @mattyg
- 1 test thread by @mattyg
- 1 thread by @mattyg
- Move json serialization tests into client library, add tests covering all supported types -- not passing yet by @mattyg
- Use ubuntu runner, specify android api level in matrix by @mattyg
- Holochain-conductor-runtime-ffi happy path tests by @mattyg
- Bad admin response error by @mattyg
- Sign zome call by @mattyg
- Happy path tests passing, generate seed bundle if not found when creating conductor by @mattyg

### Refactor

- \[**BREAKING**\] Rename the plugin to tauri-plugin-hc by @zippy
  - Tauri-plugin-holochain is reserved on crates.io by another owner. The crate, its directory, the JS helper package (tauri-plugin-hc-api) and the Tauri identifier all move to hc. The identifier cannot stay "holochain": tauri-build derives the ACL name from the links key, so permissions become hc:default / hc:allow-* and invokes become plugin:hc|<command>.
  - The identifier now lives in one PLUGIN_NAME constant used by Builder::new and by both webview injectors. The legacy app-websocket injector had "holochain" hardcoded in the bundle for its zome-call signer, which the rename would have broken silently; it now takes the name from Rust like the IPC injector does, and a new test fails if the bundle hardcodes any plugin name.
- *(runtime)* Drop the Android-service-only modules by @zippy
  - Authorization.rs gated cross-app access by Android package name (ClientId), autostart.rs persisted a start-on-device-boot flag for the foreground service, and persisted.rs existed only to back those two. None has a consumer in an in-process runtime: handle_app_request never consulted the authorization map (the caller scopes the app id), so the manager Runtime held was inert weight.
  - These move with the Android service stack. Also removes the two now-unused PersistedFile* error variants.
- Set runtime network config in tauri plugin initialization, rather than via android-service-runtime UI -- this makes it simpler to swap tauri-plugin-holochain-service-client with tauri-plugin-holochain (embedded conductor), while preserving ability to override config via UI if desired by @mattyg
- Use md docs, committed to repo, instead of gh pages deployment of html docs by @mattyg
- Use serde_json instead of rmp-serde by @mattyg
- Extract cfg(test) condition for running in multi thread runtime into trait by @mattyg
- We cannot use Binder.getCallingUid() within the onBind callback, as it is called from the system. Instead we allow the binder to be created, but check authorization within each function by @mattyg
- Add connectSetupApp fn to make explicit that we are both connecting and setting up app, tauri plugin now exposes call that function by @mattyg
- Simplify theming implementation for disconnect notice by @mattyg
- Extract DisconnectNotice into separate class by @mattyg
- Move logic to install happ from injected JS run on webview initialization, to rust IPC calls run on tauri setup by @mattyg
- Use custom exception type when unable to connect to service, handle by displaying notice by @mattyg
- Client does not start service, service-plugin does by @mattyg
- Move json serialization helpers into client lib, use in both tauri plugins by @mattyg
- Avoid depdency on tauri subproject for json serialization by @mattyg
- Move conversion to & from parcelable types into client fn calls, avoid exposing to user of client by @mattyg
- Parcelable for InstallAppPayloadFfi, instead of toParcel/fromParcel by @mattyg
- Tauri plugins both use HolochainServiceClient, clean up aidl types and type casting by @mattyg
- Tauri plugins both use holochain-service-types kotlin lib for types by @mattyg
- Extract types from holochain_runtime_uniffi, so their FFI bindings can be generated separately for use in a common kotlin library by @mattyg
- Move tauri plugins from p2p-shipyard fork (https://github.com/mattyg/p2p-shipyard/tree/feat/foreground-service) into this repo by @mattyg

### Documentation

- *(plugin)* Note mobile window.open behavior and raise the Tauri floor to 2.11 by @zippy
- Record the startup helpers as built in the generator plan by @zippy
- Add the app generator and startup helpers plan by @zippy
- Rewrite for the cross-platform runtime, add crate metadata by @zippy
  - README covers desktop/Android/iOS, the in-process model, and points at holochain/android-service-runtime for the shared-conductor stack. The runtime crate's README no longer documents the removed autostart/authorization modules, and tauri-plugin-holochain gets the README it never had.
  - DEVELOPMENT.md drops the Kotlin and UniFFI release ceremony and records the target-cfg wasm backend rule and the frontendDist build-order gotcha.
  - Both crates gain the description/readme/repository metadata a crates.io publish requires.
- Record the iOS device run (iPhone 12 mini, iOS 18.7.8)
  - The stack runs on real hardware, not just the simulator: conductor boot, forum hApp install, zome call + signing, and an app signal all succeed.
  - The result that matters is `get_all_posts returned 3 record(s)`. On the simulator this was always 0 on a fresh data dir; three records means posts from earlier launches survived, so the conductor databases and the lair keystore persist across restarts inside the real app container -- at app_data_dir(), the path that was impossible before the socket fix. A device sandbox is genuinely enforced, so this is stronger evidence than the simulator could produce, and it confirms deleting the two path workarounds was right.
- Correct how the iOS SUN_LEN limit differs between simulator and device
  - The status summary claimed the conductor "cannot run anywhere under the iOS app container", which is wrong for a physical device and made the working simulator build look like it contradicted the analysis.
  - The two targets fail for different reasons. A device container is ~78 bytes shorter than a simulator's, so a short in-container path clears the ~104 B AF_UNIX limit there (<container>/hc/socket = 94 B) while nothing under a simulator container ever can (162 B before anything is appended). All figures now canonicalized, since lair canonicalizes the root and /var -> /private/var costs 8 bytes on device.
  - This inverts the usual expectation and is worth stating plainly: the simulator arm is the more artificial one. It runs only because simulator apps are ordinary macOS processes that can write outside their container, which a real device forbids. The device has the stricter sandbox but the shorter path and is the one that can stay inside the container -- so a green simulator run is not evidence about a device.
  - Also sharpens why the device arm still is not shippable: it is forced to put conductor data at the container root rather than Application Support (Documents/hc/socket misses by exactly one byte), so it gets no conventional backup semantics. The constraint is not "our directory name is too long", it is that a whole class of correct locations is unreachable -- which is the argument for fixing it upstream rather than tuning the path.
  - Comment-only change to lib.rs; both iOS targets still build clean.
- Plan for an iOS test build of the unified tauri-plugin-holochain by @zippy
- *(changelog)* Drop the lair error rendering entry by @zo-el
  - It reads as a cosmetic improvement to an error string, not a change a consumer needs to act on.
- Correct the Serialization doc and log the lair error change by @zo-el
  - Two review follow-ups, neither touching behaviour.
- Describe the EGL setup as host-first vendor ordering, not a software fallback by @zippy
  - Nixpkgs Mesa is the last resort in the vendor list and drives AMD/Intel GPUs in hardware; software rendering (llvmpipe) only happens on NVIDIA.
- Plan for the holochain 0.7.0 update (desktop, then android) by @zippy
  - Two phases: (1) unified tauri-plugin-holochain in the desktop build — JS client to released ^0.21.0, dist-js rebuild, and adding the plugin suite + example build to the CI-gated make integration-test; (2) the android service/client crates and Kotlin libraries — cross-compile of the iroh transport under cargo-ndk, regenerated uniffi bindings (breaking RuntimeNetworkConfigFfi change), handwritten Kotlin mirrors/tests, dokka, validated by the existing emulator CI job.
- *(deps)* Note 0.6.2-rc.0 bump; describe init_properties FFI in present tense by @zo-el
- Plan ASR additions for unyt-on-ASR by @zippy
  - Companion to unyt's unyt-admin-access-needs.md, phased to match it. unyt drives admin from its own in-process Rust commands and uses a per-window multi-network UX, so the ASR-side surface is small:
  - Phase 2 (now): a single add, Runtime::dump_network_stats(); keep   list_apps() arg-less. No admin-over-IPC, no multi-app app_request,   no client-js change. - Phase 3 (deferred, co-design): the controllable-boot cluster (late   passphrase, pre-launch lair hook for hc-auth material, restart-   keeping-lair, generate_agent_pub_key, export_agent_seed).
  - Records why the earlier admin-IPC / trusted-multi-app-window drafts were dropped (bundled-binary trust model; pinning is the future Case-2 boundary, not app_id).
- Plan for direct Tauri app-call path by @zippy
  - Phase 5 on top of the unified plugin: route @holochain/client App API calls through Tauri IPC into the in-process conductor instead of a loopback app-websocket. Detection + transport live in a new module in holochain-client-js; the plugin gains an app_request command and window-scoped app binding. Records the design, the AppInterfaceApi / subscribe_to_app_signals findings, the security model, and phasing.
- Plan for Holochain 0.6.1 update and unified FFI-free Tauri plugin by @zippy

### Automated Changes

- *(deps)* Bump rust-toolchain from 1.98.1 to 1.99.0 by @dependabot[bot] in [#22](https://github.com/holochain/runtime-tauri/pull/22)
- *(deps)* Bump toml from 0.9.12+spec-1.1.0 to 1.1.2+spec-1.1.0 by @dependabot[bot] in [#26](https://github.com/holochain/runtime-tauri/pull/26)
- *(deps)* Bump the cargo-minor-patch group with 6 updates by @dependabot[bot] in [#25](https://github.com/holochain/runtime-tauri/pull/25)
- *(deps)* Bump the nix group with 2 updates by @dependabot[bot] in [#24](https://github.com/holochain/runtime-tauri/pull/24)
- *(deps)* Bump the npm group with 4 updates by @dependabot[bot] in [#23](https://github.com/holochain/runtime-tauri/pull/23)
- *(deps)* Bump rust-toolchain from 1.95.0 to 1.98.1 by @dependabot[bot]
- *(deps)* Bump base64 from 0.22.1 to 0.23.1 by @dependabot[bot] in [#12](https://github.com/holochain/runtime-tauri/pull/12)
- *(deps)* Bump the cargo-minor-patch group with 10 updates by @dependabot[bot]
- *(deps)* Bump the npm group with 6 updates by @dependabot[bot]
- *(deps)* Bump the nix group with 3 updates by @dependabot[bot]
- *(deps)* Bump the updates group with 3 updates by @dependabot[bot]
- *(deps)* Bump jni from 0.21.1 to 0.22.4 by @dependabot[bot]
- Update CODEOWNERS with shared content in [#3](https://github.com/holochain/runtime-tauri/pull/3)
- Update dependabot.yml with shared content in [#2](https://github.com/holochain/runtime-tauri/pull/2)

### Other Changes

- Merge pull request #1 from holochain/ci/add-ci-pass-check by @zippy in [#1](https://github.com/holochain/runtime-tauri/pull/1)
- IOS support for the in-process runtime by @zippy
  - Brings the ios branch (wasmi backend on iOS, Xcode project, socket-free lair keystore) onto the 0.3.0 line.
- Merge pull request #146 from holochain/feat/typed-install-errors-payload-signing by @zippy
- Merge pull request #145 from holochain/main-0.7 by @zippy
- Merge pull request #143 from holochain/chore/0.7-js-client-and-ci by @zippy
- Update to Holochain 0.7 (off develop) by @zippy
- Merge chore/holochain-0.6.2-rc.0: bump to holochain 0.6.2-rc.0 by @zippy
- Merge pull request #141 from holochain/feat/tauri-window-rebind by @zippy
- Merge pull request #133 from holochain/Chore/v0.1.5 by @Amirhesp
- Bump ASR app version to 0.1.5 for release
- Merge the v0.1.3 tag's commits
- Merge pull request #132 from holochain/fix/open-settings by @mattyg
- Merge pull request #128 from holochain/update-0.5.4 by @mattyg
- Update infra urls by @zippy
- Merge pull request #125 from holochain/build/holochain-0.5 by @matthme
- Update package-lock by @matthme
- Clippy by @mattyg
- Use p2p-shipyard's nix flake by @mattyg
- Merge branch 'main' into build/holochain-0.5 by @mattyg
- Merge pull request #126 from holochain/doc/improve-docs-pass-2 by @matthme
- Link to readmes in root readme by @mattyg
- Merge pull request #118 from holochain/feat/config-on-start by @mattyg
- Merge branch 'main' into feat/config-on-start by @mattyg
- Merge pull request #123 from holochain/build/publish-tauri-plugins by @mattyg
- Merge branch 'main' into build/publish-tauri-plugins by @mattyg
- Merge pull request #122 from holochain/doc/improve-docs by @mattyg
- Improve README for all components, include doc comments for all published crates by @mattyg
- Publishing tauri plugin & dep crates by @mattyg
- Trigger ci by @mattyg
- Trigger ci by @mattyg
- Merge pull request #117 from holochain/build/fix-app-building by @mattyg
- Merge branch 'build/fix-app-building' into feat/config-on-start by @mattyg
- Merge branch 'build/fix-app-building' into feat/config-on-start by @mattyg
- Unnecessary changes by @mattyg
- Merge branch 'build/fix-app-building' into feat/config-on-start by @mattyg
- Merge branch 'build/fix-app-building' into feat/config-on-start by @mattyg
- Merge pull request #108 from holochain/feat/config-ice by @mattyg
- Merge branch 'main' into feat/config-ice by @mattyg
- Merge pull request #114 from holochain/feat/ui-translations by @mattyg
- Merge branch 'main' into feat/ui-translations by @mattyg
- Merge branch 'main' into feat/config-ice by @mattyg
- Merge pull request #111 from holochain/ci/publish-libraries by @mattyg
- Unrelated changes by @mattyg
- Trigger ci by @mattyg
- Contributing guide, releasing kotlin libs by @mattyg
- Add contributing docs, outline creating releases by @mattyg
- Merge branch 'main' into feat/config-ice by @mattyg
- Merge pull request #110 from holochain/doc/publish-docs by @mattyg
- Add gh pages links to readme by @mattyg
- Merge pull request #109 from holochain/ci/build-apks-in-ci by @mattyg
- Merge pull request #104 from holochain/chore/lint by @mattyg
- Merge branch 'main' into chore/lint by @mattyg
- Merge pull request #106 from holochain/fix/my-apps-crash by @mattyg
- Merge branch 'main' into fix/my-apps-crash by @mattyg
- Merge pull request #103 from holochain/feat/launch-on-startup by @mattyg
- Merge branch 'main' into feat/launch-on-startup by @mattyg
- Merge branch 'main' into fix/my-apps-crash by @mattyg
- Merge branch 'build/helpful-npm-commands' into feat/launch-on-startup by @mattyg
- 0.4 servers by @mattyg
- Ice urls in RuntimeConfig by @mattyg
- Merge pull request #102 from holochain/build/helpful-npm-commands by @mattyg
- Merge branch 'feat/user-approval-authorized-apps' into build/helpful-npm-commands by @mattyg
- Merge pull request #99 from holochain/feat/user-approval-authorized-apps by @mattyg
- Merge pull request #96 from holochain/chore/ktlint by @mattyg
- Trigger ci by @mattyg
- Merge branch 'refactor/extract-holochainservice-to-separate-kotlin-lib' into chore/ktlint by @mattyg
- Merge branch 'refactor/extract-holochainservice-to-separate-kotlin-lib' into chore/ktlint by @mattyg
- Readme by @mattyg
- Merge pull request #93 from holochain/refactor/extract-holochainservice-to-separate-kotlin-lib by @mattyg
- Add new library to README by @mattyg
- Temporarily remove linter from gradle build setup -- will restore in another PR by @mattyg
- Trigger ci by @mattyg
- Don't publish to mavenlocal but sync & build libs by @mattyg
- Extract HolochainService into its own kotlin library by @mattyg
- Merge pull request #90 from holochain/feat/separate-admin-app-binding-ipc-2 by @mattyg
- Don't run tests in ci yet by @mattyg
- Merge branch 'main' into feat/separate-admin-app-binding-ipc-2 by @mattyg
- Merge pull request #87 from holochain/refactor/move-setup-app-to-rust-crate by @mattyg
- More explanation of setup_app by @mattyg
- Merge branch 'main' into feat/separate-admin-app-binding-ipc-2 by @mattyg
- Merge pull request #89 from holochain/fix/delete-duplicate-lib by @mattyg
- Merge branch 'refactor/move-setup-app-to-rust-crate' into feat/separate-admin-app-binding-ipc-2 by @mattyg
- Merge branch 'main' into refactor/move-setup-app-to-rust-crate by @mattyg
- Merge branch 'main' into feat/separate-admin-app-binding-ipc-2 by @mattyg
- Merge pull request #88 from holochain/fix/package-scripts by @mattyg
- Merge branch 'refactor/move-setup-app-to-rust-crate' into feat/separate-admin-app-binding-ipc-2 by @mattyg
- Include holochain pkgs in flake for working on example-client-app for now by @mattyg
- Merge pull request #75 from holochain/feat/notice-when-service-not-connected by @mattyg
- Merge branch 'main' into feat/notice-when-service-not-connected by @mattyg
- Merge pull request #71 from holochain/fix/static-tests-separate-ci by @mattyg
- Merge branch 'main' into fix/static-tests-separate-ci by @mattyg
- Test-threads 1 by @mattyg
- Merge pull request #78 from holochain/feat/setup-app-command by @mattyg
- Merge pull request #61 from holochain/feat/example-consumer-app by @mattyg
- Merge branch 'main' into feat/example-consumer-app by @mattyg
- Merge pull request #76 from holochain/build/holochain-android-dev-nix-flake by @mattyg
- Merge branch 'main' into build/holochain-android-dev-nix-flake by @mattyg
- Merge branch 'main' into build/holochain-android-dev-nix-flake by @mattyg
- Merge branch 'main' into feat/example-consumer-app by @mattyg
- Merge pull request #70 from holochain/fix/logger-initialized-twice by @mattyg
- Merge branch 'main' into fix/logger-initialized-twice by @mattyg
- Merge branch 'main' into fix/logger-initialized-twice by @mattyg
- Merge branch 'main' into feat/example-consumer-app by @mattyg
- Merge pull request #67 from holochain/fix/json-conversion by @mattyg
- Merge branch 'main' into feat/setup-app-command by @mattyg
- Merge pull request #65 from holochain/fix/parcelable-types by @mattyg
- Merge branch 'feat/setup-app-command' into feat/notice-when-service-not-connected by @mattyg
- Merge branch 'feat/example-consumer-app' into feat/notice-when-service-not-connected by @mattyg
- Notice implementation by @mattyg
- Merge branch 'fix/json-conversion' into feat/example-consumer-app by @mattyg
- Merge branch 'main' into feat/example-consumer-app by @mattyg
- Merge pull request #68 from holochain/fix/temporary-uniffi-fork by @mattyg
- Merge branch 'fix/json-conversion' into feat/example-consumer-app by @mattyg
- Merge branch 'fix/parcelable-types' into feat/example-consumer-app by @mattyg
- Merge pull request #63 from holochain/fix/runtime-release-builds-crash-when-calling-jna-lib by @mattyg
- Merge branch 'fix/runtime-release-builds-crash-when-calling-jna-lib' into feat/example-consumer-app by @mattyg
- Merge branch 'main' into feat/example-consumer-app by @mattyg
- Merge pull request #60 from holochain/chore/cleanup-build-setup by @mattyg
- Merge pull request #57 from holochain/test/client-unit-tests by @mattyg
- Rust caching, setup rustup in separate step by @mattyg
- Do i need the rust-toolchain gh action? by @mattyg
- Build deps in ci by @mattyg
- Build deps in ci by @mattyg
- Wip by @mattyg
- Trigger Build by @mattyg
- How to run tests in readme by @mattyg
- Merge pull request #55 from holochain/feat/non-blocking-async-ipc-calls by @mattyg
- Merge pull request #53 from holochain/build/local-maven-for-development by @mattyg
- Merge pull request #52 from holochain/chore/uniffi-bindgen-separate-crate by @mattyg
- Merge pull request #49 from holochain/feat/is-conductor-running by @mattyg
- Merge pull request #22 from holochain/refactor/extract-common-code-into-libs by @mattyg
- No longer using local .aab library by @mattyg
- Note kotlin lib in readme by @mattyg
- Merge branch 'main' into refactor/extract-common-code-into-libs by @mattyg
- Merge pull request #30 from holochain/ci/tests-on-android by @mattyg
- Don't include android targets in this PR by @mattyg
- Merge pull request #28 from holochain/chore/readme-fix by @mattyg
- Merge pull request #24 from holochain/refactor/new-holochain-runtime by @mattyg
- Modify uniffi crate build scripts to output bindings & libs to kotlin library, modify tauri-plugin-holochain-service to use shared library, bump uniffi -- having an issue where uniffi constructor is not generated by @mattyg
- Merge pull request #21 from holochain/refactor/move-tauri-plugins-into-repo by @mattyg
- Merge pull request #19 from holochain/build/nix-flake by @mattyg
- Merge pull request #18 from holochain/build/nix-flake by @mattyg
- Merge pull request #17 from holochain/feat/system-settings by @mattyg
- Merge pull request #16 from holochain/feat/remove-preinstalled-apps by @mattyg

### First-time Contributors

- @mattyg made their first contribution in [#29](https://github.com/holochain/runtime-tauri/pull/29)
- @dependabot[bot] made their first contribution in [#22](https://github.com/holochain/runtime-tauri/pull/22)
- @zippy made their first contribution in [#18](https://github.com/holochain/runtime-tauri/pull/18)
- @jost-s made their first contribution in [#5](https://github.com/holochain/runtime-tauri/pull/5)
- @ made their first contribution in [#4](https://github.com/holochain/runtime-tauri/pull/4)
- @zo-el made their first contribution
- @Amirhesp made their first contribution
- @matthme made their first contribution
# Unreleased

- Upgrade to holochain v0.7.1-rc.2
- Breaking: the legacy app websocket is removed. It attached an app interface
  that accepted any origin and injected a reusable, non-expiring token, and no
  consumer still used it; windows reach the conductor over Tauri IPC only.
  `WindowOptions::use_app_websocket` and the injected `__HC_LAUNCHER_ENV__` are
  gone, and `holochain-conductor-runtime` drops `Runtime::ensure_app_websocket`,
  `Runtime::setup_app` and `AppAuth`. Call `Runtime::install_app_if_missing`
  where you called `setup_app`.
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
- `create-holochain-tauri` ships a restrictive `csp` instead of `csp: null`:
  script from the app only plus `'wasm-unsafe-eval'` for libsodium, no remote
  or inline script, `style-src-attr 'unsafe-inline'` because Tauri's style
  nonces would otherwise cancel `'unsafe-inline'` for `style` attributes, and
  `connect-src` limited to Tauri IPC (the legacy `use_app_websocket` path also
  needs `ws://localhost:*`). Tauri only applies a CSP to pages it serves, so
  `tauri dev` against a Vite `devUrl` runs without one. The example app ships
  the same policy, which its dev build does apply, and turns `withGlobalTauri`
  off, invoking its `report` command through `@tauri-apps/api` instead.
- Startup helpers, extracted from Emergence so each app stops carrying its own
  copy: `tauri_plugin_hc::app_paths` (production and per-dev-instance data
  directories), `UserNetworkConfig` with the `get_`/`default_`/`set_user_network_config`
  app commands, and `on_ready`, which runs startup work once the conductor is up
  even if it came up first. `holochain-conductor-runtime` adds
  `Runtime::install_app_if_missing`, which `setup_app` now uses, and fails boot
  with `DataRootPathTooLong` when lair's socket path would exceed the Unix socket
  limit instead of lair's `path must be shorter than SUN_LEN`.
- `get_`, `default_` and `set_user_network_config` are plugin commands now
  (`plugin:hc|…`), so Tauri's ACL applies to them. `hc:default` includes the
  two reads; `set`, which repoints bootstrap and relay and restarts, needs
  `hc:allow-set-user-network-config` on the window that hosts the settings
  screen. Apps drop them from `generate_handler!`, keep
  `.manage(UserNetworkConfigPath(..))`, and invoke them with the `plugin:hc|`
  prefix; emergence does all three when it next bumps the plugin (Rust fails
  to compile until then, the Svelte `invoke` calls fail at run time). Calling
  them without that state is `Error::UserNetworkConfigPathNotManaged` rather
  than a panic. `set` now requests the restart (`request_restart`), so it
  returns `Ok` to the UI and the app exits through `RunEvent::ExitRequested`
  and `Exit` instead of skipping them.
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
  origin excepted, so exports still work), and on desktop `window.open` and
  `target="_blank"` open nothing. Tauri has no `on_new_window` on mobile: there
  Android loads the target in the same webview, where the navigation check
  applies, and iOS opens nothing. `HolochainPlugin::lock_navigation` applies
  the policy to a window the app builds itself; `navigation_allowed`,
  `origin_of` and `same_origin` are public. `on_new_window` arrived in Tauri
  2.8; the workspace floor moves from 2.5.1 to 2.11, the version CI builds.
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
