#!/usr/bin/env node
import { parseArgs } from "node:util";
import { init } from "../lib/init.js";

const USAGE = `Usage: npm create holochain-tauri -- [options]
       npx create-holochain-tauri [init] [options]

Adds a Tauri desktop and Android app to the hApp repository in the current
directory, built on runtime-tauri's tauri-plugin-hc.

Options:
  --app-id <id>             installed app id (default: the hApp name)
  --product-name <name>     window title and bundle name (default: the app id)
  --identifier <id>         bundle identifier (default: org.holochain.<app id>)
  --ui <workspace>          npm workspace of the UI (default: ui)
  --happ-manifest <path>    hApp manifest (default: workdir/happ.yaml)
  --runtime-tauri <path>    depend on a local runtime-tauri checkout instead of GitHub
  --force                   overwrite an existing src-tauri
  --skip-install            do not run npm install
  -h, --help                show this help`;

let parsed;
try {
  parsed = parseArgs({
    allowPositionals: true,
    options: {
      "app-id": { type: "string" },
      "product-name": { type: "string" },
      identifier: { type: "string" },
      ui: { type: "string" },
      "happ-manifest": { type: "string" },
      "runtime-tauri": { type: "string" },
      force: { type: "boolean" },
      "skip-install": { type: "boolean" },
      help: { type: "boolean", short: "h" },
    },
  });
} catch (e) {
  console.error(`${e.message}\n\n${USAGE}`);
  process.exit(2);
}

const { values, positionals } = parsed;
const command = positionals[0] ?? "init";

if (values.help) {
  console.log(USAGE);
  process.exit(0);
}
if (command === "update") {
  console.error("`update` is not implemented yet.");
  process.exit(1);
}
if (command !== "init") {
  console.error(`Unknown command "${command}".\n\n${USAGE}`);
  process.exit(2);
}

try {
  const { notes } = init({
    root: process.cwd(),
    appId: values["app-id"],
    productName: values["product-name"],
    identifier: values.identifier,
    ui: values.ui,
    happManifest: values["happ-manifest"],
    runtimeTauri: values["runtime-tauri"],
    force: values.force,
    install: !values["skip-install"],
  });

  console.log(`
Next steps:
  nix develop
  npm run start:tauri          two desktop agents on a local network

For Android, generate the Android project once, then run a desktop and an
Android agent together:
  nix develop .#androidDev
  npm run tauri android init
  npm run network:android`);
  if (notes.length > 0) {
    console.log(`\nNotes:\n${notes.map((n) => `  - ${n}`).join("\n")}`);
  }
} catch (e) {
  console.error(`create-holochain-tauri: ${e.message}`);
  process.exit(1);
}
