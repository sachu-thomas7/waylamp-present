#!/usr/bin/env node
// Generates src-tauri/resources/THIRD_PARTY_NOTICES.txt: shipped Rust crates (normal deps, Windows target),
// production npm deps of both frontends, PDFium and WebView2. Usage: node scripts/gen-notices.cjs
const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const { execFileSync } = require("child_process");

const ROOT = path.resolve(__dirname, "..");
const OUT = path.join(ROOT, "src-tauri", "resources", "THIRD_PARTY_NOTICES.txt");
const TARGET = process.env.NOTICES_TARGET || "x86_64-pc-windows-msvc";

const entries = []; // { kind, name, version, license, texts[] }

function licenseTexts(dir) {
  const texts = [];
  let files = [];
  try { files = fs.readdirSync(dir); } catch { return texts; }
  for (const f of files) {
    if (/^(licen[cs]e|copying|notice|unlicense)([-_.].*)?$/i.test(f) && fs.statSync(path.join(dir, f)).isFile()) {
      texts.push(fs.readFileSync(path.join(dir, f), "utf8").replace(/\r\n/g, "\n").trim());
    }
  }
  return texts;
}

// Rust: walk the resolve graph from the app crate along normal (non-dev, non-build) edges.
const meta = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--locked", "--filter-platform", TARGET], {
    cwd: ROOT, maxBuffer: 256 * 1024 * 1024, encoding: "utf8",
  }),
);
const pkgs = new Map(meta.packages.map((p) => [p.id, p]));
const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
const rootId = meta.workspace_members.find((id) => pkgs.get(id).name === "waylamp-present");
const seen = new Set();
(function walk(id) {
  if (seen.has(id)) return;
  seen.add(id);
  for (const d of nodes.get(id).deps) {
    if (d.dep_kinds.some((k) => k.kind === null)) walk(d.pkg);
  }
})(rootId);
for (const id of seen) {
  if (id === rootId) continue;
  const p = pkgs.get(id);
  entries.push({
    kind: "Rust crate", name: p.name, version: p.version,
    license: p.license || `see ${p.license_file}`, texts: licenseTexts(path.dirname(p.manifest_path)),
  });
}

// npm: production dependency closure of each frontend.
for (const proj of ["controller", "presentation"]) {
  const base = path.join(ROOT, proj);
  const pj = JSON.parse(fs.readFileSync(path.join(base, "package.json"), "utf8"));
  const done = new Set();
  const visit = (name, from) => {
    let dir = from;
    let found = null;
    for (;;) {
      const cand = path.join(dir, "node_modules", name);
      if (fs.existsSync(path.join(cand, "package.json"))) { found = cand; break; }
      const up = path.dirname(dir);
      if (up === dir || up.length < base.length) break;
      dir = up;
    }
    if (!found) return; // optional/platform dep not installed
    const real = fs.realpathSync(found);
    if (done.has(real)) return;
    done.add(real);
    const dp = JSON.parse(fs.readFileSync(path.join(real, "package.json"), "utf8"));
    entries.push({
      kind: `npm (${proj})`, name: dp.name, version: dp.version,
      license: typeof dp.license === "string" ? dp.license : JSON.stringify(dp.license || dp.licenses || "UNKNOWN"),
      texts: licenseTexts(real),
    });
    for (const dep of Object.keys(dp.dependencies || {})) visit(dep, real);
  };
  for (const dep of Object.keys(pj.dependencies || {})) visit(dep, base);
}

const unknown = entries.filter((e) => /UNKNOWN/.test(e.license));
if (unknown.length) {
  console.error("licence missing for:", unknown.map((e) => `${e.name}@${e.version}`).join(", "));
  process.exit(1);
}
entries.sort((a, b) => (a.kind + a.name + a.version).localeCompare(b.kind + b.name + b.version));

const deps = JSON.parse(fs.readFileSync(path.join(ROOT, "release", "deps.json"), "utf8")).pdfium;
const pdfiumLic = path.join(ROOT, "src-tauri", "resources", "pdfium", "PDFIUM-LICENSE.txt");
const pdfiumText = fs.existsSync(pdfiumLic) ? fs.readFileSync(pdfiumLic, "utf8").replace(/\r\n/g, "\n").trim() : "(run scripts/fetch-deps.sh first)";

let out = "";
out += "Waylamp Present is licensed under GPL-3.0-or-later (see LICENSE.txt).\n";
out += "It includes the third-party components below under their own licences.\n\n";
out += "=== PDFium " + deps.version + " (" + deps.source + ", chromium/" + deps.tag.split("/")[1] + ") ===\n";
out += "Bundled unmodified as pdfium.dll. PDFium is licensed BSD-3-Clause / Apache-2.0; its bundled licence file follows.\n\n" + pdfiumText + "\n\n";
out += "=== Microsoft Edge WebView2 Runtime ===\n";
out += "The installer carries Microsoft's WebView2 Runtime installer (offline) and installs it only when the PC lacks a recent runtime.\n";
out += "It is Microsoft software under the WebView2 Runtime licence terms (https://developer.microsoft.com/microsoft-edge/webview2), not covered by the GPL.\n\n";
out += `=== ${entries.length} Rust crates and npm packages ===\n\n`;
for (const e of entries) out += `${e.name} ${e.version}  [${e.kind}]  ${e.license}\n`;
out += "\n=== Licence texts (grouped; identical texts are listed once) ===\n";
const groups = new Map();
for (const e of entries) {
  for (const t of e.texts) {
    const h = crypto.createHash("sha256").update(t).digest("hex");
    if (!groups.has(h)) groups.set(h, { text: t, who: [] });
    groups.get(h).who.push(`${e.name} ${e.version}`);
  }
}
for (const g of groups.values()) {
  out += "\n----------------------------------------\nApplies to: " + g.who.join(", ") + "\n\n" + g.text + "\n";
}
fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, out);
console.log(`wrote ${path.relative(ROOT, OUT)}: ${entries.length} components, ${groups.size} distinct licence texts, ${out.length} bytes`);
