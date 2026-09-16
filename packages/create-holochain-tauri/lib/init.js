import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { newFlake, updateScaffoldFlake } from "./flake.js";

const TEMPLATES = join(dirname(fileURLToPath(import.meta.url)), "..", "templates");

/** Port the UI dev server listens on; tauri.conf.json's devUrl points at it. */
export const UI_PORT = 1420;

const DEV_DEPENDENCIES = {
  "@tauri-apps/cli": "^2.11.0",
  concurrently: "^8.2.2",
  "concurrently-repeat": "^0.0.1",
  "get-port-cli": "^3.0.0",
  "internal-ip-cli": "^2.0.0",
};

/**
 * Add a Tauri desktop/Android app to the hApp repository at `root`.
 *
 * @param {object} options
 * @param {string} options.root app repository root
 * @param {string} [options.appId] installed app id; defaults to the hApp name
 * @param {string} [options.productName] window title and bundle name; defaults to the app id
 * @param {string} [options.identifier] reverse-DNS bundle identifier
 * @param {string} [options.ui] npm workspace of the UI; defaults to "ui"
 * @param {string} [options.happManifest] path to happ.yaml; defaults to "workdir/happ.yaml"
 * @param {string} [options.runtimeTauri] local runtime-tauri checkout to depend on instead of GitHub
 * @param {boolean} [options.force] overwrite an existing src-tauri
 * @param {boolean} [options.install] run npm install afterwards
 * @param {(line: string) => void} [options.log]
 * @returns {{ notes: string[] }}
 */
export function init(options) {
  const root = resolve(options.root);
  const log = options.log ?? console.log;
  const notes = [];

  const packageJsonPath = join(root, "package.json");
  if (!existsSync(packageJsonPath)) {
    throw new Error(`No package.json in ${root}. Run this from the hApp repository root.`);
  }
  const happManifest = join(root, options.happManifest ?? "workdir/happ.yaml");
  if (!existsSync(happManifest)) {
    throw new Error(`No hApp manifest at ${relative(root, happManifest)}. Pass --happ-manifest.`);
  }
  const happName = readHappName(happManifest);
  const ui = options.ui ?? "ui";
  if (!existsSync(join(root, ui, "package.json"))) {
    throw new Error(`No UI package at ${ui}/package.json. Pass --ui with the UI's npm workspace.`);
  }
  const srcTauri = join(root, "src-tauri");
  if (existsSync(srcTauri) && !options.force) {
    throw new Error("src-tauri already exists. Pass --force to overwrite the files this writes.");
  }

  const appId = options.appId ?? happName;
  const values = {
    appId,
    productName: options.productName ?? appId,
    identifier: options.identifier ?? defaultIdentifier(appId),
    version: readJson(join(root, ui, "package.json")).version ?? "0.1.0",
    crateName: `${toKebab(appId)}-tauri`,
    libName: `${toSnake(appId)}_lib`,
    happBundlePath: `${relative(root, dirname(happManifest))}/${happName}.happ`,
    uiWorkspace: ui,
    uiPort: String(UI_PORT),
    pluginDependency: pluginDependency(options.runtimeTauri),
  };

  copyTemplates(join(TEMPLATES, "src-tauri"), srcTauri, values);
  log("wrote src-tauri/");

  const scripts = addPackageJsonEntries(packageJsonPath, { appId, ui });
  log(`package.json: added ${scripts.added.length} scripts and the Tauri dev dependencies`);
  for (const name of scripts.skipped) {
    notes.push(`package.json already has a "${name}" script, so it was left as it is.`);
  }

  const flakeUrl = runtimeTauriFlakeUrl(options.runtimeTauri);
  const flakePath = join(root, "flake.nix");
  if (!existsSync(flakePath)) {
    writeFileSync(flakePath, newFlake({ runtimeTauriFlakeUrl: flakeUrl, appId }));
    log("wrote flake.nix");
    notes.push("flake.nix is new: `git add flake.nix` before `nix develop`, which only sees tracked files.");
  } else {
    const result = updateScaffoldFlake(readFileSync(flakePath, "utf8"), {
      runtimeTauriFlakeUrl: flakeUrl,
      appId,
    });
    if (result.status === "updated") {
      writeFileSync(flakePath, result.contents);
      log("updated flake.nix to build on runtime-tauri's dev shells");
      notes.push("Review the flake.nix change, then run `nix flake lock` (or `nix develop`) to lock runtime-tauri.");
    } else if (result.status === "already") {
      log("flake.nix already uses runtime-tauri; left unchanged");
    } else {
      notes.push(
        `flake.nix was not changed: ${result.reason}. Update it by hand as described under "Dev shell" in the create-holochain-tauri README.`,
      );
    }
  }

  if (appendGitignore(join(root, ".gitignore"), ["/src-tauri/target/"])) {
    log("added /src-tauri/target/ to .gitignore");
  }

  if (options.install) {
    log("running npm install");
    execFileSync("npm", ["install"], { cwd: root, stdio: "inherit" });
  }

  return { notes };
}

export function readHappName(manifestPath) {
  const match = readFileSync(manifestPath, "utf8").match(/^name:\s*['"]?([^'"\s#]+)/m);
  if (!match) {
    throw new Error(`Could not read the hApp name from ${manifestPath}`);
  }
  return match[1];
}

export function defaultIdentifier(appId) {
  // Android package names allow only letters, digits and underscores per segment.
  const segment = appId.toLowerCase().replace(/[^a-z0-9]/g, "");
  return `org.holochain.${/^[a-z]/.test(segment) ? segment : `app${segment}`}`;
}

function pluginDependency(runtimeTauri) {
  if (runtimeTauri) {
    const path = join(resolve(runtimeTauri), "crates", "tauri-plugin-hc");
    return `{ path = ${JSON.stringify(path)} }`;
  }
  return `{ git = "https://github.com/holochain/runtime-tauri" }`;
}

function runtimeTauriFlakeUrl(runtimeTauri) {
  return runtimeTauri ? `git+file://${resolve(runtimeTauri)}` : "github:holochain/runtime-tauri";
}

/** Scripts for the Tauri dev loop. Existing scripts with the same name are kept. */
export function tauriScripts({ appId, ui }) {
  const uiServer = `UI_PORT=${UI_PORT} npm run start -w ${ui} -- --host 0.0.0.0 --strictPort`;
  return {
    "start:tauri": "AGENTS=${AGENTS:-2} BOOTSTRAP_PORT=$(get-port) npm run network:tauri",
    "network:tauri": `npm run build:happ && INTERNAL_IP=$(internal-ip --ipv4) concurrently -k "npm run local-services" "${uiServer}" "npm run launch:tauri"`,
    "start:android": `npm run build:happ && BOOTSTRAP_PORT=$(get-port) INTERNAL_IP=$(internal-ip --ipv4) concurrently -k "npm run local-services" "${uiServer}" "npm run launch:android"`,
    "network:android": `npm run build:happ && BOOTSTRAP_PORT=$(get-port) INTERNAL_IP=$(internal-ip --ipv4) concurrently -k "npm run local-services" "${uiServer}" "AGENTS=1 npm run launch:tauri" "npm run launch:android"`,
    "launch:tauri": 'concurrently-repeat "npm run tauri dev" $AGENTS',
    "launch:android":
      'adb start-server && until adb devices | grep -q "[[:space:]]device$"; do echo "waiting for an Android device or emulator..."; sleep 2; done && tauri android dev ${ANDROID_DEVICE:-}',
    "local-services": "kitsune2-bootstrap-srv --listen 0.0.0.0:$BOOTSTRAP_PORT",
    tauri: "tauri",
    "clean:tauri-dev": `rm -rf ~/.cache/${appId}/holochain-*`,
  };
}

function addPackageJsonEntries(path, { appId, ui }) {
  const raw = readFileSync(path, "utf8");
  const pkg = JSON.parse(raw);
  const added = [];
  const skipped = [];

  pkg.scripts ??= {};
  for (const [name, command] of Object.entries(tauriScripts({ appId, ui }))) {
    if (name in pkg.scripts) {
      if (pkg.scripts[name] !== command) skipped.push(name);
    } else {
      pkg.scripts[name] = command;
      added.push(name);
    }
  }

  pkg.devDependencies ??= {};
  for (const [name, version] of Object.entries(DEV_DEPENDENCIES)) {
    pkg.devDependencies[name] ??= version;
  }

  writeFileSync(path, JSON.stringify(pkg, null, 2) + "\n");
  return { added, skipped };
}

function appendGitignore(path, entries) {
  const existing = existsSync(path) ? readFileSync(path, "utf8") : "";
  const lines = new Set(existing.split("\n").map((l) => l.trim()));
  const missing = entries.filter((e) => !lines.has(e));
  if (missing.length === 0) return false;
  const prefix = existing === "" || existing.endsWith("\n") ? "" : "\n";
  writeFileSync(path, `${existing}${prefix}\n# Tauri\n${missing.join("\n")}\n`);
  return true;
}

function copyTemplates(from, to, values) {
  mkdirSync(to, { recursive: true });
  for (const entry of readdirSync(from, { withFileTypes: true })) {
    const source = join(from, entry.name);
    if (entry.isDirectory()) {
      copyTemplates(source, join(to, entry.name), values);
    } else if (entry.name.endsWith(".tmpl")) {
      const target = join(to, entry.name.slice(0, -".tmpl".length));
      writeFileSync(target, render(readFileSync(source, "utf8"), values));
    } else {
      cpSync(source, join(to, entry.name));
    }
  }
}

export function render(template, values) {
  return template.replace(/\{\{(\w+)\}\}/g, (_, key) => {
    if (!(key in values)) throw new Error(`template value "${key}" is not set`);
    return values[key];
  });
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function toKebab(s) {
  return s.replace(/[^A-Za-z0-9]+/g, "-").replace(/^-|-$/g, "").toLowerCase();
}

function toSnake(s) {
  return s.replace(/[^A-Za-z0-9]+/g, "_").replace(/^_|_$/g, "").toLowerCase();
}
