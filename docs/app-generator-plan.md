# App generator and startup helpers plan

Status: decided, not yet implemented (except `dev_network_config`, see §4.2). Read
this whole document before writing code.

Decisions:

- The generator lives in this repository as the npm package `create-holochain-tauri`,
  released together with `tauri-plugin-hc`. There is no separate kangaroo-tauri
  repository.
- `npm create holochain-tauri` creates or adopts an app; `npx create-holochain-tauri update`
  rewrites the files the generator owns.
- Startup support is plain helper functions in `tauri-plugin-hc` that the app calls
  from its own code. A launcher that sequences them behind hooks is deferred (§4.3).

## 1. Why

A Holochain app that wants a desktop + Android + iOS build on the in-process Tauri
runtime currently has to hand-write a Tauri shell. Two apps have done so, and they
are copies of one another that have already drifted apart:

- Emergence (branch `tauri-plugin-hc`) — ported to `tauri-plugin-hc`; runs on desktop,
  an Android emulator and an Android phone, with a desktop and a phone agent syncing
  over a local dev network.
- kando (branch `main-0.7`) — still on darksoil's `tauri-plugin-holochain` 0.6 API;
  its shell is otherwise the same design.

Normalizing the app name, their `src-tauri/src/lib.rs` files (~285 lines each) have the
same function set and differ in 193 lines, all of it API/version drift rather than
app-specific logic. Their `.github/workflows` differ by 2–14 lines, and their npm
scripts differ only where Emergence received fixes that kando did not.

The generator should make a Holochain app repository look like Emergence's layout
without each app owning a copy of the shell logic that then drifts.

This is **not** the `kangaroo-electron` model. `kangaroo-electron` is a wrapper
repository around a prebuilt `.webhapp` in `pouch/`. That model cannot provide the dev
experience wanted here — a desktop agent and an Android agent, both hot-reloading,
against a local network (`npm run network:android`) — because that needs the UI source,
its dev server and the zome sources in the same repository. A pouch mode could be added
later for packaging third-party `.webhapp`s; it is out of scope now.

## 2. What exists

- `crates/runtime` — `holochain-conductor-runtime`, framework-free conductor wrapper.
- `crates/tauri-plugin-hc` — the Tauri plugin. Identifier `hc` (`hc:default`,
  `plugin:hc|…`). Webview env `__HC_TAURI_HOLOCHAIN__`, events
  `holochain://ready|lair-ready|setup-failed|signal|rebound`. Serves the App API over
  Tauri IPC; `@holochain/client` 0.21 `AppWebsocket.connect()` detects it.
- `apps/holochain-runtime-example` — minimal reference app, desktop/Android/iOS.
- `flake.nix` exports `devShells.tauriDev` and `devShells.tauriAndroidDev` for apps to
  compose.

Commits that encode lessons the generator must carry: `b245937` rename to
tauri-plugin-hc · `460f6ac` npm workspaces · `d87b17e` example installs/enables
directly (no app websocket) · `868739b` + `5de4c67` dev profile (`opt-level = 3`,
`debug = false` for dependencies) · `e330298` exported dev shells · `e647b2b` Android
`ndk_context` initialized in `JNI_OnLoad` · `58a3394` Linux camera access ·
`5e53434` `dev_network_config` / `dev_network_url!`.

## 3. Architecture

| Layer | Where | Changes reach apps by |
| --- | --- | --- |
| Conductor + plugin | `crates/` | crate version bump |
| Startup helpers (§4) | `tauri-plugin-hc` | crate version bump |
| Repository layout, config, scripts, CI, flake (§5) | `packages/create-holochain-tauri` | `npx create-holochain-tauri update` |
| Identity, icons, splash, UI, app-specific startup work | the app | — |

After generation, the only Rust an app owns in `src-tauri` is what is genuinely about
that app, plus the short startup wiring that calls the helpers.

### 3.1 Generator, not a fork-this template

Emergence and kando already exist; a GitHub template cannot be merged into an existing
repository, and "fork and merge upstream" is how kando and Emergence drifted. The
generator runs inside a scaffolded hApp repository and writes the files it owns (§5),
leaving app-owned files alone. `update` rewrites owned files and reports conflicts in
files the app modified. It is an npm package because every app already has Node, and
it is versioned with the plugin so generated files always match the plugin they use.

### 3.2 Configuration lives in `tauri.conf.json`

Do not add a separate config file. Tauri plugins read their own section
(`plugins.hc`) natively, and identity (`identifier`, `productName`, `version`) already
lives in `tauri.conf.json`. Put **data** there (ids, URLs, sizes, flags); put
**behavior** in the app's Rust. Where fields overlap with `kangaroo-electron`'s
`kangaroo.config.ts`, use the same names (`appId`, `bootstrapUrl`, `relayUrl`,
`passwordMode`, …) so moving an app between the two is mechanical.

The hApp bytes are embedded with `include_bytes!`, which needs a compile-time path.
Either keep that one line in the app's `lib.rs` or generate a `build.rs` that reads the
path from config; do not try to read the bundle path at runtime on mobile.

## 4. Startup helpers

Apps need to do arbitrary work during startup, so **nothing in the library owns
`tauri::Builder`, `.setup()`, `invoke_handler` or `.run()`.** The library contributes
plugins, commands and functions the app calls, in the order it chooses.

### 4.1 What apps actually do at startup

From Emergence and kando's shells, plus needs that are already visible:

| Step | Emergence / kando today | Other apps will need |
| --- | --- | --- |
| Tauri builder | extra plugins (log, opener, os), own commands | tray, deep links, notifications, any plugin |
| Passphrase | empty passphrase at plugin init | prompt the user first (`passwordMode`), keychain |
| Data directory | per-instance dev dirs with lock files; UserCache in dev, UserData in prod | profiles, custom locations |
| Network config | defaults → local dev bootstrap → user overrides file | hc-auth, per-profile network seed, UI-driven URLs |
| Before install | — | import/restore an agent key (`restore_from_dht`), fetch a membrane proof |
| Install | install if missing, then enable (Emergence); plus update coordinators when the bundle changed (kando) | several apps, no install until the user chooses, role settings |
| After install | — | create clone cells, start Rust-side signal listeners |
| Windows | open main window (size), close splashscreen | several windows, app-less dashboard + `rebind_window`, no window (tray) |
| Failure | `exit(1)` on `holochain://setup-failed` | show an error window, retry, offer to reset |

### 4.2 Helpers

Each is independently usable, has no hidden ordering, and takes plain inputs. Extract
them from Emergence's working `lib.rs`, which is the most debugged source, with unit
tests:

- **Done:** `dev_network_config(url)` and `dev_network_url!()` — local bootstrap/relay,
  plain-text relay, raised gossip burst limit, and the rule that every agent on a dev
  network uses the same URL (§6.7, §6.8).
- `user_network` — persisted user network config (get/default/set commands, applied on
  top of the base config).
- `paths::data_dir(app, opts)` — dev instance numbering with lock files, production
  location, mobile via Tauri's path API, and a length check against the ~108-byte
  AF_UNIX limit (§6.9).
- `install::ensure(runtime, app_id, bundle, opts) -> Outcome { Installed, Present, Upgraded }`
  — install-if-missing + enable, with an opt-in "update coordinators when the bundle
  hash changed" (kando's `update_app_if_necessary`).
- `window::open_main(plugin, app_id, opts)` and `window::close(app, label)`.

The generated `lib.rs` calls them from the app's own `.setup()`:

```rust
.setup(|app| {
    let handle = app.handle().clone();
    app.listen(EVENT_READY, move |_| {
        let handle = handle.clone();
        tauri::async_runtime::spawn(async move {
            let rt = handle.holochain()?.runtime();
            tauri_plugin_hc::install::ensure(rt, APP_ID, HAPP_BUNDLE_BYTES).await?;
            // app-specific work goes here, as ordinary code
            tauri_plugin_hc::window::open_main(&handle, APP_ID, |w| w.inner_size(1200.0, 880.0)).await?;
            Ok(())
        });
    });
    Ok(())
})
```

### 4.3 Deferred: a launcher

A `Launcher` that runs the whole sequence with a hook at each step (network,
passphrase, before/after install, install strategy, main window, error) would shrink
each app's wiring further. Build it only if the hooks cover Emergence, kando and an app
that waits for a passphrase from the UI before booting, all without library changes,
forks, or global state. Apps could then move to it without losing any helper.

## 5. What the generator writes (Emergence-shaped layout)

Owned by the generator (rewritten by `update`):

- `src-tauri/Cargo.toml` — separate workspace root, excluded from the zome workspace,
  its own `Cargo.lock`; `holochain = { version = "…", default-features = false, features = ["encryption", "schema"] }`
  (the runtime picks the wasm backend per target; default features would link the JIT
  into iOS); `tauri-plugin-hc`; and:
  ```toml
  [profile.dev.package."*"]
  opt-level = 3
  debug = false
  ```
- `src-tauri/src/main.rs`, `build.rs`, and the initial `lib.rs` (§4.2; app-owned after
  `init`).
- `src-tauri/capabilities/*.json` with `core:default` and `hc:default`.
- `src-tauri/tauri.conf.json` skeleton with a `plugins.hc` section (identity fields are
  filled once at `init`, then app-owned).
- `src-tauri/gen/android` and `gen/apple` — **generated with the current Tauri CLI at
  `init` time** (`tauri android init`, `tauri ios init`), then patched. Never carried as
  files in this repository.
- `flake.nix` dev shells: `default` = runtime-tauri's `tauriDev` + app extras,
  `androidDev` = `tauriAndroidDev` + app extras, with the input
  `runtime-tauri = { url = "github:holochain/runtime-tauri"; inputs.holonix.follows = "holonix"; }`.
- `package.json` scripts: `start:tauri` (N desktop agents), `start:android`,
  `network:android` (1 desktop + 1 Android, both hot-reloading), `local-services`,
  `launch:tauri`, `launch:android`, `clean:tauri-dev`.
- `.github/workflows/release-tauri-app.yaml` and the Tauri part of `test.yaml`.

App-owned (written once at `init`, never overwritten): identity in `tauri.conf.json`,
icons, splashscreen HTML, UI code, extra capabilities, `lib.rs` after `init`.

Not the generator's business: zomes, the hApp's release process (Emergence and kando
freeze a canonical `.happ` and verify its sha256 — the generated workflow should
consume that pattern, not redefine it).

## 6. Lessons from getting Emergence running — each must be in the generated output

Every item below cost real debugging time. Encode each one; do not rediscover them.

1. **Dev shell.** Build `src-tauri` only inside a shell that provides webkit from nix.
   Emergence's old shell used nix's gcc (glibc 2.42 headers) but found webkit in the host
   `/usr/lib` (glibc 2.35), and linking failed with `undefined symbol: __isoc23_strtol`.
   Mixed builds poison `target/`; the fix required `cargo clean`.
2. **Dev profile.** Without `opt-level = 3` for dependencies a new cell's first zome
   call (wasm compile + `init`) took 29.6 s vs 2.9 s, conductor boot 19.7 s vs 1.7 s.
   Without `debug = false` the Android debug library was 1,313 MB (1,180 MB DWARF) and
   the APK failed with `INSTALL_FAILED_INSUFFICIENT_STORAGE`; with it, 268 MB.
3. **Android project versions.** A 2024 `gen/android` (Kotlin plugin 1.6.21, AGP 8.3.2,
   Gradle 8.4) cannot compile against Tauri 2.11's AndroidX/`kotlin-stdlib` 2.0.21.
   Current CLI output uses Kotlin 1.9.25, AGP 8.5.1, Gradle 8.9.
4. **Package-manager runner baked into mobile projects.** `tauri android init` / `ios init`
   write the runner (`pnpm tauri …` vs `npm run tauri -- …`) into the Gradle task and the
   Xcode build phase. Generate with the package manager the app uses (npm).
5. **Device selection.** `launch:android` waits in a loop until `adb devices` lists a
   device (`adb wait-for-device` fails when several are attached), then runs
   `tauri android dev ${ANDROID_DEVICE:-}`. Without a selected device Tauri silently
   builds `aarch64` and then tries to open Android Studio. Tauri matches `ANDROID_DEVICE`
   against the device name prefix (e.g. `Volla`), not the adb serial.
6. **`ndk_context`.** Tauri 2.11 (tao 0.35) no longer initializes it; `app_dirs2`, iroh's
   DNS resolver and `netdev` panic on Android without it. Fixed in `tauri-plugin-hc`
   (`e647b2b`, `JNI_OnLoad`). Apps must not define their own `JNI_OnLoad`.
7. **Bootstrap server reachability.** `local-services` must bind `0.0.0.0`; on Android the
   host address and port are compiled in, falling back to `10.0.2.2` (the emulator's host
   alias). `dev_network_url!()` handles this.
8. **One relay URL per dev network.** Every agent must advertise the *same* bootstrap/relay
   URL. A desktop agent on `http://127.0.0.1:<port>` and an Android agent on
   `http://<LAN IP>:<port>` share a server, but kitsune2 rejects the peer:
   `Peer is on unknown relay, failing preflight`. Desktop dev agents use `INTERNAL_IP`
   when set. Plain-text relay needs `advanced.irohTransport.relayAllowPlainText = true`.
   `dev_network_config` sets it.
9. **Unix socket path length.** Lair binds a socket under the data root; paths over
   ~108 bytes fail with `path must be shorter than SUN_LEN` (desktop too, not only iOS).
10. **Frozen hApp.** Emergence and kando never rebuild the canonical `.happ` (the wasm
    embeds absolute paths, so a rebuild is a different DNA). Dev scripts that run
    `build:happ` overwrite the on-disk copy; releases must download the canonical bytes.
11. **UI integration.** `AppWebsocket.connect()` needs nothing extra under Tauri, but
    signal handlers must use the `@holochain/client` ≥ 0.19 shape
    `{ type: "app", value: { cell_id, zome_name, payload } }`. Emergence's 2023 handler
    silently dropped every signal.
12. **Clean dev slate** means both `~/.cache/<app>/holochain-*` (conductors) and
    `~/.local/share/<identifier>` (webview `localStorage`, shared by all dev instances
    because they load the same dev-server origin).
13. **Hot reload scope.** `tauri android dev` watches `src-tauri`, not path dependencies
    outside it; edits in a local runtime-tauri checkout need a restart.
14. **Emulator.** The nix Android SDK ships no emulator; use the host SDK's. A wedged
    emulator (`adb shell` hanging, boot animation spinning forever) needed
    `emulator -avd <name> -wipe-data -no-snapshot`.
15. **iOS** builds and runs on device/simulator only with an unreleased socket-free lair
    keystore (see `docs/ios-test-build-plan.md`); generate `gen/apple` but do not put iOS
    in CI yet.
16. **Gossip burst limit on dev networks.** On kitsune2's default `initiateBurstFactor`
    a phone agent dropped a desktop agent's gossip rounds and new data took minutes to
    arrive; `dev_network_config` raises it for dev. The cause is still under
    investigation, so production stays on the default.
17. **Camera on Linux.** QR scanning needs WebKitGTK media permission (granted by the
    plugin, `58a3394`) and GStreamer capture plugins from the dev shell.

## 7. Order of work

1. Push this repository; switch Emergence's `tauri-plugin-hc` dependency and flake input
   to `github:holochain/runtime-tauri`; restore Emergence's canonical `.happ`; commit its
   branch.
2. Helpers (§4.2) in `tauri-plugin-hc`, extracted from Emergence; port Emergence's
   `lib.rs` to them.
3. `packages/create-holochain-tauri` (§5), using Emergence's files as the golden output.
   Its CI test: generate into a fresh `hc-scaffold` example hApp and into a copy of
   Emergence, then build desktop and Android.
4. Port kando with the generator; the diff against Emergence's shell should shrink to
   app-owned files.

## 8. Open questions

- How `update` reports and resolves conflicts in owned files the app has edited.
- Whether mobile dev agents should hold DHT data (kando's old config set
  `target_arc_factor = 0` on mobile).
