import { copyFileSync, existsSync, unlinkSync, renameSync, readFileSync, readdirSync, mkdirSync } from "fs";
import { execSync } from "child_process";
import { join } from "path";
import { fileURLToPath } from "url";
import { dirname } from "path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const root = join(__dirname, "..");
const srcTauri = join(root, "src-tauri");

const tauriConf = JSON.parse(readFileSync(join(srcTauri, "tauri.conf.json"), "utf-8"));
const version = tauriConf.version;
const productName = tauriConf.productName;

const archMap = { x64: "x64", arm64: "aarch64" };
const targetArch = process.env.TAURI_ENV_TARGET_TRIPLE?.includes("aarch64")
  ? "aarch64"
  : process.env.TARGET?.includes("aarch64")
    ? "aarch64"
    : process.arch in archMap
      ? archMap[process.arch]
      : "x64";
const detectedArch = targetArch;

const nsisDir = join(srcTauri, "target", "release", "nsis", detectedArch);
if (!existsSync(nsisDir)) {
  const altDir = join(srcTauri, "target", "release", "nsis", "x64");
  if (existsSync(altDir)) {
    console.log(`Architecture dir '${detectedArch}' not found, falling back to x64`);
  }
}
const resolvedNsisDir = existsSync(nsisDir)
  ? nsisDir
  : join(srcTauri, "target", "release", "nsis", "x64");

const customLangDir = join(srcTauri, "nsis-lang");
const makensisDir =
  process.env.NSIS_HOME ||
  process.env.MAKENSIS ||
  join(process.env.LOCALAPPDATA || "", "tauri", "NSIS");
const makensis = join(makensisDir, "makensis.exe");

const langFiles = ["SimpChinese.nsh", "English.nsh"];

for (const file of langFiles) {
  const src = join(customLangDir, file);
  const dest = join(resolvedNsisDir, file);
  if (!existsSync(src)) {
    console.log(`Custom language file not found: ${src}, skipping`);
    continue;
  }
  if (!existsSync(dest)) {
    console.log(`Generated language file not found: ${dest}, skipping`);
    continue;
  }
  copyFileSync(src, dest);
  console.log(`Patched: ${file}`);
}

if (!existsSync(makensis)) {
  console.error(`makensis not found at: ${makensis}`);
  process.exit(1);
}

const installerNsi = join(resolvedNsisDir, "installer.nsi");
if (!existsSync(installerNsi)) {
  console.error(`installer.nsi not found at: ${installerNsi}`);
  process.exit(1);
}

console.log("Re-compiling NSIS installer...");
execSync(`"${makensis}" /INPUTCHARSET UTF8 "${installerNsi}"`, { stdio: "inherit" });

const nsisOutput = join(resolvedNsisDir, "nsis-output.exe");
const bundleDir = join(srcTauri, "target", "release", "bundle", "nsis");

if (!existsSync(nsisOutput)) {
  console.error("nsis-output.exe not found after compilation");
  process.exit(1);
}

const setupPattern = `${productName}_${version}_`;
const files = readdirSync(bundleDir);
const setupFile = files.find((f) => f.startsWith(setupPattern) && f.endsWith("-setup.exe"));

if (!setupFile) {
  console.error(`No setup exe matching '${setupPattern}*-setup.exe' found in ${bundleDir}`);
  console.error(`Available files: ${files.join(", ")}`);
  process.exit(1);
}

const setupExe = join(bundleDir, setupFile);
console.log(`Target setup exe: ${setupFile}`);

try {
  unlinkSync(setupExe);
} catch {}
renameSync(nsisOutput, setupExe);
console.log(`Updated: ${setupExe}`);

const releaseDir = join(root, "release");
mkdirSync(releaseDir, { recursive: true });
const releaseDest = join(releaseDir, setupFile);
copyFileSync(setupExe, releaseDest);
console.log(`\nCopied to: ${releaseDest}`);
