import { execSync } from "child_process";
import { existsSync, mkdirSync, renameSync, copyFileSync, readFileSync, lstatSync } from "fs";
import { join } from "path";
import { platform, arch } from "os";
import https from "https";
import http from "http";
import { createWriteStream, unlinkSync } from "fs";
import { createGunzip } from "zlib";
import { createHash } from "crypto";

const NODE_VERSION = "v22.23.2";

const NODE_SHA256 = {
  "node-v22.23.2-darwin-arm64.tar.gz":
    "61130f394c1630d211dd50aecc4353d379480f36d3ac913cd85dbba1aed585c6",
  "node-v22.23.2-darwin-x64.tar.gz":
    "58e99022c2ff89395576cc7fd4d98cea24bb68081475d5f88b801ee8729fb026",
  "node-v22.23.2-linux-arm64.tar.gz":
    "013b59cfd2819703a6f4a14ab891fc46fc2a4e3f5bcd92de3fb4929b43e35b30",
  "node-v22.23.2-linux-x64.tar.gz":
    "b294a556e639d64338823920e5866c21c02741742d2e1529ee1a225c1ec9252a",
  "win-arm64/node.exe": "97cce5301a815d2dce07ac5bfd1e6039eae88185ec1d10ae4f8cb712f1732878",
  "win-x64/node.exe": "0d0f5e39f9f3d9587bc19f73eab3c2c9c4903fd02d6dbf9c853dd81b3d95fad4",
};

const PLATFORM_MAP = {
  win32: { os: "win", ext: ".exe" },
  darwin: { os: "darwin", ext: "" },
  linux: { os: "linux", ext: "" },
};

const ARCH_MAP = {
  x64: "x64",
  arm64: "arm64",
};

const TRIPLE_MAP = {
  "win32-x64": "x86_64-pc-windows-msvc",
  "win32-arm64": "aarch64-pc-windows-msvc",
  "darwin-x64": "x86_64-apple-darwin",
  "darwin-arm64": "aarch64-apple-darwin",
  "linux-x64": "x86_64-unknown-linux-gnu",
  "linux-arm64": "aarch64-unknown-linux-gnu",
};

function downloadFile(url, dest, maxRedirects = 5) {
  return new Promise((resolve, reject) => {
    if (maxRedirects <= 0) return reject(new Error("Too many redirects"));
    const proto = url.startsWith("https") ? https : http;
    const req = proto.get(url, (response) => {
      if (response.statusCode === 301 || response.statusCode === 302) {
        const location = response.headers.location;
        if (!location || !location.startsWith("https://")) {
          return reject(new Error(`Invalid or non-HTTPS redirect: ${location}`));
        }
        return downloadFile(location, dest, maxRedirects - 1).then(resolve, reject);
      }
      if (response.statusCode !== 200) {
        reject(new Error(`HTTP ${response.statusCode}`));
        return;
      }
      const file = createWriteStream(dest);
      response.on("error", reject);
      file.on("error", reject);
      response.pipe(file);
      file.on("finish", () => {
        file.close();
        resolve();
      });
    });
    req.setTimeout(60000, () => {
      req.destroy();
      reject(new Error("Download timeout"));
    });
    req.on("error", reject);
  });
}

function verifyChecksum(filePath, expectedHash) {
  const data = readFileSync(filePath);
  const actual = createHash("sha256").update(data).digest("hex");
  if (actual !== expectedHash) {
    throw new Error(
      `Checksum mismatch for ${filePath}:\n  expected: ${expectedHash}\n  actual:   ${actual}`,
    );
  }
  console.log(`Checksum verified: ${filePath}`);
}

async function main() {
  const os = platform();
  const cpuArch = arch();
  const platInfo = PLATFORM_MAP[os];
  const archStr = ARCH_MAP[cpuArch];
  const triple = TRIPLE_MAP[`${os}-${cpuArch}`];

  if (!platInfo || !archStr || !triple) {
    console.error(`Unsupported platform: ${os}-${cpuArch}`);
    process.exit(1);
  }

  console.log(`Platform: ${os}-${cpuArch} (${triple})`);
  console.log(`Node.js version: ${NODE_VERSION}`);

  const binDir = join("src-tauri", "binaries");
  const resDir = join("src-tauri", "resources", "whistle");

  mkdirSync(binDir, { recursive: true });
  mkdirSync(resDir, { recursive: true });

  // Step 1: Download Node.js binary
  const nodeBinName = `node-${triple}${platInfo.ext}`;
  const nodeBinPath = join(binDir, nodeBinName);

  let needsDownload = !existsSync(nodeBinPath);
  if (!needsDownload) {
    try {
      if (os === "win32") verifyChecksum(nodeBinPath, NODE_SHA256[`win-${archStr}/node.exe`]);
      const versionOut = execSync(`"${nodeBinPath}" -v`, {
        encoding: "utf8",
        timeout: 10000,
      }).trim();
      if (versionOut !== NODE_VERSION) {
        console.log(
          `Node.js version mismatch: have ${versionOut}, need ${NODE_VERSION}. Re-downloading...`,
        );
        unlinkSync(nodeBinPath);
        needsDownload = true;
      }
    } catch {
      console.log("Cannot verify existing Node.js binary, re-downloading...");
      try {
        unlinkSync(nodeBinPath);
      } catch {}
      needsDownload = true;
    }
  }
  if (needsDownload) {
    console.log("Downloading Node.js binary...");

    if (os === "win32") {
      const nodeUrl = `${process.env.NODE_DOWNLOAD_MIRROR || "https://nodejs.org/dist"}/${NODE_VERSION}/win-${archStr}/node.exe`;
      console.log(`URL: ${nodeUrl}`);
      await downloadFile(nodeUrl, nodeBinPath);
      const checksumKey = `win-${archStr}/node.exe`;
      if (NODE_SHA256[checksumKey]) verifyChecksum(nodeBinPath, NODE_SHA256[checksumKey]);
    } else {
      const nodeArchive = `node-${NODE_VERSION}-${platInfo.os}-${archStr}.tar.gz`;
      const nodeUrl = `${process.env.NODE_DOWNLOAD_MIRROR || "https://nodejs.org/dist"}/${NODE_VERSION}/${nodeArchive}`;
      const archivePath = join(binDir, nodeArchive);

      console.log(`URL: ${nodeUrl}`);
      await downloadFile(nodeUrl, archivePath);
      if (NODE_SHA256[nodeArchive]) verifyChecksum(archivePath, NODE_SHA256[nodeArchive]);

      console.log("Extracting Node.js binary...");
      execSync(
        `tar xzf "${archivePath}" -C "${binDir}" --strip-components=2 "node-${NODE_VERSION}-${platInfo.os}-${archStr}/bin/node"`,
        { stdio: "inherit" },
      );

      const extractedNode = join(binDir, "node");
      if (existsSync(extractedNode)) {
        renameSync(extractedNode, nodeBinPath);
        execSync(`chmod +x "${nodeBinPath}"`);
      }

      try {
        unlinkSync(archivePath);
      } catch {}
    }

    console.log(`Node.js binary saved to: ${nodeBinPath}`);
  } else {
    console.log(`Node.js binary already exists (${NODE_VERSION}): ${nodeBinPath}`);
  }

  // Step 2: Install whistle (with lockfile for reproducible builds)
  const whistleModules = join(resDir, "node_modules", "whistle");
  {
    const lockfilePath = join(resDir, "package-lock.json");
    if (!existsSync(lockfilePath)) {
      throw new Error(
        `Missing ${lockfilePath}. Generate and commit lockfile before build to keep sidecar dependencies reproducible.`,
      );
    }
    console.log("Installing whistle (npm ci)...");
    execSync(`npm ci --ignore-scripts --no-audit --no-fund --prefix "${resDir}"`, {
      stdio: "inherit",
      env: { ...process.env, NODE_ENV: "production" },
    });
    console.log("Whistle installed successfully.");
  }

  // Remove npm self-link junction to prevent infinite recursion in tauri_build resource scanning
  const selfLink = join(resDir, "node_modules", "whistlebox-whistle");
  if (existsSync(selfLink)) {
    if (!lstatSync(selfLink).isSymbolicLink()) throw new Error("Unexpected self-link directory");
    unlinkSync(selfLink);
    console.log("Removed npm self-link junction: whistlebox-whistle");
  }

  console.log("\nSidecar preparation complete!");
  console.log(`  Node.js: ${nodeBinPath}`);
  console.log(`  Whistle: ${whistleModules}`);
}

main().catch((err) => {
  console.error("Preparation failed:", err);
  process.exit(1);
});
