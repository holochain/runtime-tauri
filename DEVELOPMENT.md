# Contributing

## Bumping the Holochain version

1. Update the holochain dependencies in the top-level `Cargo.toml`.
2. Run `nix flake update` before `nix develop` to pull the matching Holochain
   toolchain. For a major version you will also need to change `holonix.url` in
   `flake.nix` first (e.g. `main-0.7` → `main-0.8`).
3. Make whatever changes the new Holochain API requires. `make test` builds both
   crates and runs their suites.
4. For a major version the test fixture usually has to be rebuilt. Use the
   scaffolding tool from the matching Holochain release to regenerate
   `crates/runtime/fixtures/forum.happ`: run `hc-scaffold example`, follow the
   instructions to initialize the generated repo, then `npm run package`.

The wasm execution backend is chosen per target in `crates/runtime/Cargo.toml`,
not by a cargo feature — `wasmer-sys-cranelift` everywhere except iOS, which
gets `wasmer-wasmi` because it forbids JIT. It has to be a target cfg because
`tauri ios dev/build` offers `--features` but no `--no-default-features`, so a
feature-based default would silently link the JIT into the iOS binary. This is
why `holochain` is declared `default-features = false` at the workspace root and
every member re-adds `encryption` + `schema`.

## Dev builds are optimized for dependencies

The root `Cargo.toml` sets `[profile.dev.package."*"] opt-level = 3` and
`debug = false`. The second keeps Android debug libraries installable: with
dependency debug info they are around 1.3 GB, almost all of it DWARF. Without it
the conductor, wasmer/cranelift and lair run unoptimized in dev and test builds,
and a new cell's first zome call (wasm compile plus `init()`) takes around 30 s
instead of about 3 s; conductor boot goes from under 2 s to about 20 s. Only
dependencies are optimized, so rebuilding after editing a workspace crate stays
fast. The first build after a clean takes several minutes longer.

Profiles only take effect in a workspace root, so every app that depends on
`tauri-plugin-hc` has to set this itself; the plugin README says so.

## Testing

`make test` is what CI runs: `cargo fmt --check`, `cargo clippy -Dwarnings`, the
`holochain-conductor-runtime` and `tauri-plugin-hc` suites, and a build of
the example app. The suites boot real conductors, so expect a few minutes.

The example app's UI must be built before the Rust build will succeed — Tauri
resolves `frontendDist` at compile time via `generate_context!`. `make test` and
the `npm run start:*` scripts do this for you; a bare `cargo build -p
holochain-runtime-example` will not.

## Releasing

1. Bump the crate versions in `crates/runtime/Cargo.toml` and
   `crates/tauri-plugin-hc/Cargo.toml`, and the path-dependency version
   the plugin declares for the runtime.
2. Add a `CHANGELOG.md` entry.
3. Create and push a git tag to trigger the release CI.
