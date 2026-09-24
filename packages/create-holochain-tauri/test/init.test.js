import assert from "node:assert/strict";
import { cpSync, existsSync, mkdtempSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { updateScaffoldFlake } from "../lib/flake.js";
import { defaultIdentifier, init, readHappName } from "../lib/init.js";

const FIXTURE = fileURLToPath(new URL("./fixtures/scaffold", import.meta.url));

function scaffoldCopy() {
  const dir = mkdtempSync(join(tmpdir(), "create-holochain-tauri-"));
  cpSync(FIXTURE, dir, { recursive: true });
  renameSync(join(dir, "gitignore"), join(dir, ".gitignore"));
  return dir;
}

const read = (dir, path) => readFileSync(join(dir, path), "utf8");

test("init adds a Tauri app to an hc-scaffold hApp", () => {
  const dir = scaffoldCopy();
  const { notes } = init({ root: dir, runtimeTauri: "/opt/runtime-tauri", log: () => {} });

  for (const path of [
    "src-tauri/Cargo.toml",
    "src-tauri/build.rs",
    "src-tauri/src/main.rs",
    "src-tauri/src/lib.rs",
    "src-tauri/capabilities/main.json",
    "src-tauri/tauri.conf.json",
    "src-tauri/icons/icon.png",
  ]) {
    assert.ok(existsSync(join(dir, path)), `${path} should exist`);
  }

  const lib = read(dir, "src-tauri/src/lib.rs");
  assert.match(lib, /const APP_ID: &str = "forum";/);
  assert.match(lib, /include_bytes!\("\.\.\/\.\.\/workdir\/forum\.happ"\)/);
  assert.doesNotMatch(lib, /\{\{/);

  const cargo = read(dir, "src-tauri/Cargo.toml");
  assert.match(cargo, /name = "forum-tauri"/);
  assert.match(cargo, /name = "forum_lib"/);
  assert.match(cargo, /tauri-plugin-hc = \{ path = "\/opt\/runtime-tauri\/crates\/tauri-plugin-hc" \}/);
  assert.match(read(dir, "src-tauri/src/main.rs"), /forum_lib::run\(\);/);

  const conf = JSON.parse(read(dir, "src-tauri/tauri.conf.json"));
  assert.equal(conf.identifier, "org.holochain.forum");
  assert.equal(conf.build.devUrl, "http://localhost:1420");
  assert.equal(conf.build.frontendDist, "../ui/dist");
  // The example app is where the policy actually runs (its dev build serves the
  // embedded UI, so Tauri applies the CSP); the template ships the same one.
  const exampleConf = JSON.parse(
    readFileSync(
      new URL("../../../apps/holochain-runtime-example/src-tauri/tauri.conf.json", import.meta.url),
      "utf8",
    ),
  );
  assert.match(conf.app.security.csp, /script-src 'self' 'wasm-unsafe-eval'/);
  assert.deepEqual(conf.app.security, exampleConf.app.security);

  const pkg = JSON.parse(read(dir, "package.json"));
  assert.match(pkg.scripts["network:android"], /launch:android/);
  assert.equal(pkg.scripts.start, "AGENTS=${AGENTS:-2} npm run network", "existing scripts are kept");
  assert.equal(pkg.devDependencies.concurrently, "^6.5.1", "existing dependency versions are kept");
  assert.ok(pkg.devDependencies["@tauri-apps/cli"]);

  const flake = read(dir, "flake.nix");
  assert.match(flake, /url = "git\+file:\/\/\/opt\/runtime-tauri";/);
  assert.match(flake, /devShells\.androidDev = appShell inputs'\.runtime-tauri\.devShells\.tauriAndroidDev;/);
  assert.match(flake, /nodejs_22/, "the scaffold's packages are kept");

  assert.match(read(dir, ".gitignore"), /^\/src-tauri\/target\/$/m);
  assert.ok(notes.some((n) => n.includes("nix flake lock")));
});

test("init refuses to overwrite src-tauri without --force", () => {
  const dir = scaffoldCopy();
  init({ root: dir, log: () => {} });
  assert.throws(() => init({ root: dir, log: () => {} }), /src-tauri already exists/);
  init({ root: dir, force: true, log: () => {} });
});

test("an existing script with a different command is reported, not replaced", () => {
  const dir = scaffoldCopy();
  const pkg = JSON.parse(read(dir, "package.json"));
  pkg.scripts["start:tauri"] = "echo mine";
  writeFileSync(join(dir, "package.json"), JSON.stringify(pkg));

  const { notes } = init({ root: dir, log: () => {} });
  assert.equal(JSON.parse(read(dir, "package.json")).scripts["start:tauri"], "echo mine");
  assert.ok(notes.some((n) => n.includes('"start:tauri"')));
});

test("a flake that is not hc-scaffold's is left unchanged", () => {
  const dir = scaffoldCopy();
  const custom = "{ outputs = _: { }; }\n";
  writeFileSync(join(dir, "flake.nix"), custom);

  const { notes } = init({ root: dir, log: () => {} });
  assert.equal(read(dir, "flake.nix"), custom);
  assert.ok(notes.some((n) => n.includes("flake.nix was not changed")));
});

test("a flake already on runtime-tauri is left unchanged", () => {
  const flake = read(FIXTURE, "flake.nix");
  const first = updateScaffoldFlake(flake, { runtimeTauriFlakeUrl: "github:holochain/runtime-tauri", appId: "forum" });
  assert.equal(first.status, "updated");
  const second = updateScaffoldFlake(first.contents, { runtimeTauriFlakeUrl: "github:holochain/runtime-tauri", appId: "forum" });
  assert.equal(second.status, "already");
});

test("hApp names and identifiers", () => {
  assert.equal(readHappName(join(FIXTURE, "workdir/happ.yaml")), "forum");
  assert.equal(defaultIdentifier("my-app"), "org.holochain.myapp");
  assert.equal(defaultIdentifier("2fa"), "org.holochain.app2fa");
});
