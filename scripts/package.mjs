import { execFileSync } from "node:child_process";

const bundles = { win32: "nsis", darwin: "app,dmg", linux: "deb,appimage" }[process.platform];
if (!bundles) throw new Error(`Unsupported platform: ${process.platform}`);

execFileSync(process.execPath, ["scripts/prepare-sidecar.mjs"], { stdio: "inherit" });
execFileSync(
  process.execPath,
  ["node_modules/@tauri-apps/cli/tauri.js", "build", "--bundles", bundles],
  {
    stdio: "inherit",
  },
);
if (process.platform === "win32") {
  execFileSync(process.execPath, ["scripts/patch-nsis-lang.mjs"], { stdio: "inherit" });
}
