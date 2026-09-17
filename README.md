# Holochain Tauri Runtime

A Tauri-based runtime for building Holochain apps that run on desktop, Android and iOS from one codebase.

An app built on it links a Holochain conductor directly into its own binary — one Rust process, with no separate service and no cross-process IPC. The webview talks to the conductor over Tauri IPC, so `@holochain/client` in the UI works without a loopback websocket or an open admin port.

```
your Tauri app → tauri-plugin-hc → holochain-conductor-runtime → conductor
```

## What's here

| Crate | |
| --- | --- |
| [holochain-conductor-runtime](./crates/runtime) | Framework-free wrapper around the Holochain conductor. Two-phase boot (lair first, then the conductor on that same keystore), app install/enable/disable/uninstall, app websockets, zome-call and payload signing, key generation and seed import/export, hc-auth, network stats. Talks to the conductor through `AdminInterfaceApi`/`AppInterfaceApi` in-process — it never opens an admin websocket. |
| [tauri-plugin-hc](./crates/tauri-plugin-hc) | The Tauri integration, and the runtime's only consumer here. Boots the conductor, binds webview windows to installed apps, forwards signals, serves the App API over Tauri IPC, and signs zome calls for the UI. |

[create-holochain-tauri](./packages/create-holochain-tauri) adds a desktop and Android app built on the plugin to an existing hApp repository, such as one from `hc-scaffold`: `npm create holochain-tauri`.

[apps/holochain-runtime-example](./apps/holochain-runtime-example) is a working app for all three platforms: it boots a conductor, installs the bundled `forum.happ` fixture, and opens a window connected to it.

The plugin injects a `__HC_TAURI_HOLOCHAIN__` env into each window it opens. The UI reads it and connects with `@holochain/client` over Tauri IPC; the older loopback-app-websocket path is still selectable per window.

## Platform support

| | Status |
| --- | --- |
| Desktop (Linux, macOS, Windows) | Supported. Tests run here. |
| Android | Supported. |
| iOS | Builds and runs — with one unreleased dependency, see below. |

iOS was verified end to end on an iPhone 12 mini (iOS 18.7.8) and an iPhone 17 Pro simulator: conductor boot, hApp install, zome call, signing, and an app signal, persisting across restarts. It needs holochain's in-process lair keystore to stop binding a unix socket — an iOS app-container path blows past the ~104-byte `AF_UNIX` limit. That change is **not upstream yet**, so iOS will not boot against released holochain 0.7.0. See [docs/ios-test-build-plan.md](./docs/ios-test-build-plan.md) §4.0 and "Upstream issues to file → A".

Two other iOS notes: the conductor runs under holochain's `wasmi` interpreter rather than the cranelift JIT (iOS forbids JIT), selected by target cfg in [crates/runtime/Cargo.toml](./crates/runtime/Cargo.toml); and iOS suspends backgrounded apps, so the conductor pauses when the app is not in front.

## Getting started

The build needs a pinned Rust toolchain with the Android and iOS targets, the Android SDK/NDK, Holochain dev tools, and the Tauri desktop libraries. The Nix flake provides all of it:

```sh
nix develop
npm ci
```

Run the example:

```sh
npm run start:example          # desktop
npm run start:example-android  # Android device or emulator
npm run start:example-ios      # iOS device or simulator (macOS host)
```

Run what CI runs — formatting, clippy, the runtime, plugin and generator test suites, and a build of the example:

```sh
npm run ci
```

## Origins

runtime-tauri grew out of [holochain/android-service-runtime](https://github.com/holochain/android-service-runtime), whose history it keeps.

## Development

[DEVELOPMENT.md](./DEVELOPMENT.md) covers bumping the Holochain version, regenerating test fixtures, and releasing.
