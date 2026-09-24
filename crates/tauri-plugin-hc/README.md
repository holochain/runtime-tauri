# tauri-plugin-hc

A Tauri plugin that runs a Holochain conductor **in-process** — no separate service, no cross-process IPC. One Rust binary, on desktop, Android and iOS.

It is built on [`holochain-conductor-runtime`](../runtime) and exposes it through the `HolochainExt` trait on Tauri's `App`, `AppHandle` and `Window`.

## What it does

- Unlocks the lair keystore and boots the conductor, emitting `holochain://lair-ready`, then `holochain://ready` — or `holochain://setup-failed` with the cause.
- Opens webview windows bound to an installed app. The injected `__HC_TAURI_HOLOCHAIN__` env lets `@holochain/client` reach the conductor over Tauri IPC, with no loopback websocket; the older app-websocket path stays available per window via `WindowOptions`.
- Forwards each bound app's conductor signals to its window as `holochain://signal`.
- Moves a window between installed apps in place with `rebind_window`, without recreating the OS window. A monotonic `seq` on the `holochain://rebound` event makes out-of-order delivery safe, and a failed rebind keeps the prior binding.
- Signs zome calls for the UI, and arbitrary payloads via `sign_payload`.

## Permissions

`hc:default` grants `allow-sign-zome-call` and `allow-app-request`. `sign_payload` is deliberately outside it: `sign_zome_call` signs the hash of a well-formed `ZomeCallParams`, so what it produces is only usable as the call it describes, while `sign_payload` signs caller-chosen bytes with no such domain separation. A capability has to name `hc:allow-sign-payload` itself.

The plugin identifier `hc` is what goes in capability files and `plugin:hc|…` invokes. The webview-facing names are unchanged Holochain names rather than plugin names: the injected global is `__HC_TAURI_HOLOCHAIN__` (which `@holochain/client` looks for) and events use the `holochain://` scheme.

Every hApp's `tauri.conf.json` should also set a `csp`; the one `create-holochain-tauri` writes allows script from the app itself only (plus WebAssembly, which `@holochain/client`'s libsodium needs) and connections to Tauri's IPC. That is the layer that keeps an injection in the UI from reaching the plugin's commands at all. Tauri applies it to pages it serves itself, so a `tauri dev` run against a Vite `devUrl` runs without one; the example app has no `devUrl` and is where the policy gets exercised.

## Build profile

Add this to your app's workspace root `Cargo.toml` (profiles set in a member crate
are ignored):

```toml
[profile.dev.package."*"]
opt-level = 3
debug = false
```

Without it, `tauri dev` runs the conductor, wasmer/cranelift and lair unoptimized.
A new cell's first zome call, which compiles the zomes and runs `init()`, then
takes around 30 s instead of about 3 s, and conductor boot around 20 s instead of
under 2 s. Your own crate stays unoptimized, so rebuilds after editing it are
still fast.

`debug = false` matters on Android: with dependency debug info, the debug native
library is about 1.3 GB (1.18 GB of it DWARF) and fails to install on an emulator
with `INSTALL_FAILED_INSUFFICIENT_STORAGE`. Dependency symbols are kept, so
backtraces still show function names.

## Startup helpers

Functions an app calls from its own `run()` and `.setup()`; none of them owns the
Tauri builder, so anything else the app needs at startup goes in between.

```rust
let paths = tauri_plugin_hc::app_paths(APP_ID, env!("CARGO_PKG_AUTHORS"))?;

tauri::Builder::default()
    .manage(tauri_plugin_hc::UserNetworkConfigPath(paths.user_network_config.clone()))
    .invoke_handler(tauri::generate_handler![
        tauri_plugin_hc::get_user_network_config,
        tauri_plugin_hc::default_user_network_config,
        tauri_plugin_hc::set_user_network_config,
    ])
    .plugin(tauri_plugin_hc::init(
        vec_to_locked(vec![]),
        HolochainPluginConfig::new(paths.holochain_dir.clone(), network_config(&paths)),
    ))
    .setup(|app| {
        tauri_plugin_hc::on_ready(app.handle(), |handle| async move {
            let plugin = handle.holochain()?;
            plugin.runtime().install_app_if_missing(payload, true).await?;
            plugin.main_window_builder("main", Some(APP_ID.into()), Default::default())
                .await?
                .build()?;
            Ok::<_, anyhow::Error>(())
        });
        Ok(())
    })
```

- `app_paths` picks the conductor data directory and the saved network settings
  file. Production uses the user data directory. Dev uses the user cache
  directory with one `holochain-<n>` directory per running instance, claimed with
  a lock file, so several dev agents can run side by side. It rejects a data
  directory too long for lair's socket (about 107 bytes).
- `UserNetworkConfig::apply_saved` overrides the app's bootstrap and relay URLs
  with ones the user saved. `get_user_network_config`,
  `default_user_network_config` and `set_user_network_config` are app commands
  for a settings screen; setting restarts the app.
- `on_ready` runs startup work once the conductor is up, including when it came
  up before `on_ready` was called, and only once.
- `Runtime::install_app_if_missing` installs and enables the hApp on first run
  and leaves it alone after that.

## Local development network

For dev builds, point every agent at one `kitsune2-bootstrap-srv` on your machine
(the `tauriDev` shell provides it) instead of the public bootstrap and relay
servers:

```rust
let network = if tauri::is_dev() {
    tauri_plugin_hc::dev_network_config(&tauri_plugin_hc::dev_network_url!())
} else {
    NetworkConfig::default()
};
```

Run the server with `kitsune2-bootstrap-srv --listen 0.0.0.0:$BOOTSTRAP_PORT`, and
when a phone or emulator joins, set `INTERNAL_IP` to your machine's LAN address
before building both the desktop and the mobile app.

`dev_network_url!()` builds `http://<INTERNAL_IP>:<BOOTSTRAP_PORT>`. Desktop reads
the variables at run time; Android and iOS bake them in at compile time, since the
app runs on another device. Every agent must get the same URL, because kitsune2
refuses peers on a relay it doesn't know, so a desktop on `127.0.0.1` can't reach a
phone on the LAN address.

`dev_network_config` uses that URL for both the bootstrap server and the relay,
allows the local relay's plain `http://`, and raises kitsune2's gossip burst limit
(`initiateBurstFactor` 3 → 100). With the default limit, a desktop and a phone
agent on one dev network saw the phone drop the desktop's gossip rounds and new
data take minutes to arrive; with 100 there were no drops and sync took about
90 s. Small, constantly restarted dev networks are where this limit bites.
Leave production on the default until the cause is understood.

## Usage

See [apps/holochain-runtime-example](../../apps/holochain-runtime-example) for a complete app covering all three platforms.
