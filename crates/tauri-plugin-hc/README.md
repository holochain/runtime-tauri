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

## Usage

See [apps/holochain-runtime-example](../../apps/holochain-runtime-example) for a complete app covering all three platforms.
