// Rewrites the flake.nix that `hc-scaffold` generates so its dev shells build on
// runtime-tauri's, which add the Rust targets, webkit/GTK and (for Android) the
// SDK/NDK that a Tauri build needs. Any other flake is left alone: the README
// describes the change for a person or agent to make by hand.

const HOLONIX_URL = "github:holochain/holonix?ref=main-0.7";

/**
 * @param {string} flake contents of an existing flake.nix
 * @param {{ runtimeTauriFlakeUrl: string, appId: string }} options
 * @returns {{ status: "updated", contents: string } | { status: "already" } | { status: "unrecognized", reason: string }}
 */
export function updateScaffoldFlake(flake, { runtimeTauriFlakeUrl, appId }) {
  if (flake.includes("runtime-tauri")) {
    return { status: "already" };
  }

  // The exact shape hc-scaffold writes: a single default shell built from
  // holonix's, with a package list and a shell hook.
  const shell = flake.match(
    /devShells\.default\s*=\s*pkgs\.mkShell\s*\{\s*inputsFrom\s*=\s*\[\s*inputs'\.holonix\.devShells\.default\s*\];\s*packages\s*=\s*(\(with pkgs;\s*\[[^\]]*\]\));\s*shellHook\s*=\s*''([\s\S]*?)'';\s*\};/,
  );
  if (!shell) {
    return {
      status: "unrecognized",
      reason: "its default dev shell is not the one hc-scaffold generates",
    };
  }
  if (!/holonix\.url\s*=/.test(flake) || !/flake-parts\.follows\s*=\s*"holonix\/flake-parts"/.test(flake)) {
    return {
      status: "unrecognized",
      reason: "its inputs are not the ones hc-scaffold generates",
    };
  }

  // hc-scaffold nests these two levels shallower than the rewritten flake does.
  const packages = indentAfterFirstLine(shell[1].trim(), "  ");
  const shellHook = indentLines(shell[2].replace(/^\n/, "").replace(/\s+$/, ""), "  ");

  return {
    status: "updated",
    contents: renderFlake({ packages, shellHook, runtimeTauriFlakeUrl, appId }),
  };
}

/**
 * A new flake for an app that has none.
 * @param {{ runtimeTauriFlakeUrl: string, appId: string }} options
 */
export function newFlake({ runtimeTauriFlakeUrl, appId }) {
  return renderFlake({
    packages: "(with pkgs; [\n            nodejs_22\n          ])",
    shellHook: `            export PS1='\\[\\033[1;34m\\][${appId}:\\w]\\$\\[\\033[0m\\] '`,
    runtimeTauriFlakeUrl,
    appId,
  });
}

function renderFlake({ packages, shellHook, runtimeTauriFlakeUrl, appId }) {
  return `{
  description = "Flake for ${appId} development";

  # runtime-tauri's dev shells provide the Holochain tools from holonix, Rust with
  # the wasm32 and Android targets, and nix-provided webkit/GTK for the desktop app
  # (a host webkit mixed with the nix compiler fails to link). tauriAndroidDev adds
  # the Android SDK/NDK, JDK and cargo-ndk.
  #
  #   nix develop               zomes, UI and the desktop app
  #   nix develop .#androidDev  the same plus Android
  inputs = {
    holonix.url = "${HOLONIX_URL}";

    runtime-tauri = {
      url = "${runtimeTauriFlakeUrl}";
      inputs.holonix.follows = "holonix";
    };

    nixpkgs.follows = "holonix/nixpkgs";
    flake-parts.follows = "holonix/flake-parts";
  };

  outputs = inputs@{ flake-parts, ... }: flake-parts.lib.mkFlake { inherit inputs; } {
    systems = builtins.attrNames inputs.holonix.devShells;
    perSystem = { inputs', pkgs, ... }:
      let
        appShell = base: pkgs.mkShell {
          inputsFrom = [ base ];

          packages = ${packages};

          shellHook = ''
${shellHook}
          '';
        };
      in
      {
        formatter = pkgs.nixpkgs-fmt;

        devShells.default = appShell inputs'.runtime-tauri.devShells.tauriDev;
        devShells.androidDev = appShell inputs'.runtime-tauri.devShells.tauriAndroidDev;
      };
  };
}
`;
}

function indentLines(text, indent) {
  return text
    .split("\n")
    .map((line) => (line.trim() === "" ? line : indent + line))
    .join("\n");
}

function indentAfterFirstLine(text, indent) {
  const [first, ...rest] = text.split("\n");
  return [first, ...rest.map((line) => (line.trim() === "" ? line : indent + line))].join("\n");
}
