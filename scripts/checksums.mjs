import { createHash } from "node:crypto";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const files = readdirSync("release")
  .filter((name) => /\.(exe|dmg|deb|AppImage)$/.test(name))
  .sort();
if (!files.length) throw new Error("No installers found in release/");
const sums = files.map(
  (name) =>
    `${createHash("sha256")
      .update(readFileSync(join("release", name)))
      .digest("hex")}  ${name}`,
);
writeFileSync("release/SHA256SUMS.txt", sums.join("\n") + "\n");
