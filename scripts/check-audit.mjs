import { readFileSync } from "node:fs";
let failed = false;
for (const name of ["npm-audit", "whistle-audit"]) {
  const report = JSON.parse(readFileSync(`test-results/${name}.json`, "utf8"));
  if (!report.metadata) throw Error(`${name}：漏洞查询未完成`);
  for (const [pkg, entry] of Object.entries(report.vulnerabilities || {})) {
    const message = JSON.stringify({
      package: pkg,
      severity: entry.severity,
      range: entry.range,
      fix: entry.fixAvailable,
      via: entry.via,
    })
      .replaceAll("%", "%25")
      .replaceAll("\r", "%0D")
      .replaceAll("\n", "%0A");
    console.log(`::warning title=${name}::${message}`);
    if (["high", "critical"].includes(entry.severity)) failed = true;
  }
  console.log(name, report.metadata.vulnerabilities);
}
if (failed) process.exitCode = 1;
