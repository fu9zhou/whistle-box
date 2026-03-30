import { readFileSync, writeFileSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..");

const pkg = JSON.parse(readFileSync(resolve(root, "package.json"), "utf-8"));
const version = pkg.version;

// Sync to tauri.conf.json
const tauriConfPath = resolve(root, "src-tauri/tauri.conf.json");
const tauriConf = JSON.parse(readFileSync(tauriConfPath, "utf-8"));
if (tauriConf.version !== version) {
  tauriConf.version = version;
  writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + "\n");
  console.log(`[sync-version] tauri.conf.json -> ${version}`);
}

// Sync to Cargo.toml
const cargoPath = resolve(root, "src-tauri/Cargo.toml");
let cargo = readFileSync(cargoPath, "utf-8");
const updated = cargo.replace(/^version\s*=\s*"[^"]*"/m, `version = "${version}"`);
if (updated !== cargo) {
  writeFileSync(cargoPath, updated);
  console.log(`[sync-version] Cargo.toml -> ${version}`);
}
