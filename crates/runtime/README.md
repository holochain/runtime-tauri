# holochain-conductor-runtime

A wrapper around the [`holochain`](https://docs.rs/holochain/latest/holochain/) crate exposing the [`Conductor`](https://docs.rs/holochain/latest/holochain/conductor/index.html) directly, with no host-framework or platform concerns.

It calls the conductor through `AdminInterfaceApi` and `AppInterfaceApi` in-process rather than over a websocket, so no admin interface is ever exposed to the network.

What it covers:

- **Two-phase boot.** Lair is spawned first, the hc-auth flow (if configured) signs a challenge against it and injects the resulting auth material into the `NetworkConfig`, and only then is the conductor built on that same keystore. This is what makes authenticated bootstrap and relay work on a first boot.
- **App lifecycle** — install, enable, disable, uninstall, list; `install_app_if_missing` for install-if-needed plus enable, and `setup_app`, which also attaches an app websocket.
- **Signing** — zome calls, and arbitrary payloads against a caller-chosen agent key.
- **Keys** — device key derivation, agent key generation, seed import and export.
- **App API and signals** — `handle_app_request` serves the full App API in-process; `subscribe_to_app_signals` yields an app's signal stream.

`handle_app_request` does not authorize anything: the caller is responsible for scoping the `installed_app_id` to what the requester may reach.

Its consumer in this repo is [`tauri-plugin-hc`](../tauri-plugin-hc).
