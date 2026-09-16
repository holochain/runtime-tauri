# tauri-plugin-hc

A Tauri plugin that runs a Holochain conductor **in-process** — no UniFFI, no Kotlin, no separate service, no cross-process IPC. One Rust binary, on desktop, Android and iOS.

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
