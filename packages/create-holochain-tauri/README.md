# create-holochain-tauri

Adds a Tauri desktop and Android app to a Holochain hApp repository, built on
[`tauri-plugin-hc`](../../crates/tauri-plugin-hc): the conductor runs inside the app, and
the UI's `AppWebsocket.connect()` reaches it over Tauri IPC with no changes.

Run it from the root of an hApp repository, such as one created with `hc-scaffold`:

```sh
npm create holochain-tauri
```

Until the package is published, run it from a runtime-tauri checkout, and point the
generated app at that checkout too:

```sh
npx /path/to/runtime-tauri/packages/create-holochain-tauri --runtime-tauri /path/to/runtime-tauri
```

Then:

```sh
nix develop
npm run start:tauri             # two desktop agents on a local network

nix develop .#androidDev
npm run tauri android init      # once, to generate src-tauri/gen/android
npm run network:android         # a desktop agent and an Android agent
```

## What it expects

- `package.json` at the root, with the UI as an npm workspace (default `ui`, or
  `--ui <workspace>`) whose `start` script runs Vite and honours `UI_PORT`, as
  `hc-scaffold`'s does.
- An hApp manifest at `workdir/happ.yaml` (or `--happ-manifest <path>`), and a
  `build:happ` script that packs `<hApp name>.happ` next to it.

## What it changes

| File | Change |
| --- | --- |
| `src-tauri/` | Written: `Cargo.toml`, `build.rs`, `src/main.rs`, `src/lib.rs`, `capabilities/main.json`, `tauri.conf.json`, `icons/`. Refuses to overwrite an existing `src-tauri` without `--force`. |
| `package.json` | Adds the scripts below and the dev dependencies they use (`@tauri-apps/cli`, `concurrently`, `concurrently-repeat`, `get-port-cli`, `internal-ip-cli`). A script or dependency that already exists is kept; a script with a different command is reported. |
| `flake.nix` | Rewritten only if it is exactly the shape `hc-scaffold` generates; otherwise left alone and reported (see [Dev shell](#dev-shell)). Written if there is none. |
| `.gitignore` | Adds `/src-tauri/target/`. |

After writing the files it runs `npm install` (skip with `--skip-install`).

Scripts:

| Script | Runs |
| --- | --- |
| `start:tauri` | `network:tauri` with `AGENTS` (default 2) and a free bootstrap port |
| `network:tauri` | builds the hApp, then a local bootstrap/relay server, the UI dev server on port 1420, and `AGENTS` desktop agents |
| `start:android` | the same with one Android agent |
| `network:android` | the same with one desktop and one Android agent |
| `launch:tauri`, `launch:android` | start the agents; `launch:android` waits for a device and passes `ANDROID_DEVICE` (a device name prefix, e.g. `Pixel`) to `tauri android dev` |
| `local-services` | `kitsune2-bootstrap-srv` on `0.0.0.0:$BOOTSTRAP_PORT` |
| `clean:tauri-dev` | removes the dev agents' conductor data (Linux path) |

`src-tauri/src/lib.rs` is the app's from then on: it is a short use of the plugin's
startup helpers (`app_paths`, `on_ready`, `install_app_if_missing`,
`dev_network_config`, `UserNetworkConfig`), and app-specific startup work goes there.

## Dev shell

The app's dev shells must build on runtime-tauri's, which provide the Holochain tools,
Rust with the wasm32 and Android targets, nix-provided webkit/GTK for the desktop app
(a host webkit with the nix compiler fails to link), and the Android SDK/NDK.

When `flake.nix` is not the one `hc-scaffold` generates, make these changes by hand:

1. Pin holonix to the Holochain 0.7 line and add the runtime-tauri input, following
   the app's holonix:

   ```nix
   inputs = {
     holonix.url = "github:holochain/holonix?ref=main-0.7";
     runtime-tauri = {
       url = "github:holochain/runtime-tauri";   # or git+file:///path/to/runtime-tauri
       inputs.holonix.follows = "holonix";
     };
     # ...the flake's other inputs
   };
   ```

2. Base the default shell on `inputs'.runtime-tauri.devShells.tauriDev` instead of
   `inputs'.holonix.devShells.default`, keeping the shell's own packages and hook, and
   add an `androidDev` shell on `inputs'.runtime-tauri.devShells.tauriAndroidDev`:

   ```nix
   devShells.default = pkgs.mkShell {
     inputsFrom = [ inputs'.runtime-tauri.devShells.tauriDev ];
     packages = [ pkgs.nodejs_22 ];   # the shell's existing packages
   };
   devShells.androidDev = pkgs.mkShell {
     inputsFrom = [ inputs'.runtime-tauri.devShells.tauriAndroidDev ];
     packages = [ pkgs.nodejs_22 ];
   };
   ```

   Keep any other shells the flake defines.

3. Run `nix flake lock`. Nix only sees tracked files, so `git add flake.nix` first if
   it is new.

## Not yet

- `update`, to rewrite these files for a newer runtime-tauri.
- CI workflows and iOS (`gen/apple`).
- UI hot reload on Android needs Vite's HMR pointed at the host's LAN address:
  `server.hmr = { protocol: "ws", host: <LAN IP>, port: 1421 }` in the UI's Vite config.
