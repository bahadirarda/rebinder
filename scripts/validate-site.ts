import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const root = process.cwd();
const errors: string[] = [];
const requiredFiles = [
  "site/index.html",
  "site/styles.css",
  "site/script.js",
  "site/favicon.svg",
  "site/site.webmanifest",
  "site/404.html",
  "site/robots.txt",
  "site/sitemap.xml",
  "site/llms.txt",
  "site/install.sh",
  "site/install.ps1",
  "docs/assets/rebinder-hero.png",
];

for (const path of requiredFiles) {
  if (!(await Bun.file(path).exists())) errors.push(`${path}: required website source is missing`);
}

const index = await readFile("site/index.html", "utf8");
const notFound = await readFile("site/404.html", "utf8");
const styles = await readFile("site/styles.css", "utf8");
const script = await readFile("site/script.js", "utf8");
const robots = await readFile("site/robots.txt", "utf8");
const sitemap = await readFile("site/sitemap.xml", "utf8");
const llms = await readFile("site/llms.txt", "utf8");
const manifestText = await readFile("site/site.webmanifest", "utf8");

const canonicalUrl = "https://bahadirarda.github.io/rebinder/";
const socialImage = `${canonicalUrl}assets/rebinder-social-card.png`;

for (const expected of [
  `<link rel="canonical" href="${canonicalUrl}">`,
  `<meta property="og:url" content="${canonicalUrl}">`,
  `<meta property="og:image" content="${socialImage}">`,
  `<meta name="twitter:image" content="${socialImage}">`,
  `<link rel="manifest" href="site.webmanifest">`,
]) {
  if (!index.includes(expected)) errors.push(`site/index.html: missing canonical metadata ${expected}`);
}

const titles = [...index.matchAll(/<title>([^<]+)<\/title>/g)];
if (titles.length !== 1 || !titles[0]?.[1]?.includes("Rebinder")) {
  errors.push("site/index.html: expected one descriptive Rebinder title");
}

const descriptions = [...index.matchAll(/<meta\s+name="description"\s+content="([^"]+)"/g)];
const description = descriptions[0]?.[1];
if (descriptions.length !== 1 || !description || description.length < 80 || description.length > 180) {
  errors.push("site/index.html: description must be unique and between 80 and 180 characters");
}

const ids = [...index.matchAll(/\sid="([^"]+)"/g)].map((match) => match[1] as string);
const duplicateIds = ids.filter((id, indexOfId) => ids.indexOf(id) !== indexOfId);
for (const id of new Set(duplicateIds)) errors.push(`site/index.html: duplicate id ${id}`);

const headingCount = [...index.matchAll(/<h1(?:\s|>)/g)].length;
if (headingCount !== 1) errors.push(`site/index.html: expected one h1, found ${headingCount}`);

for (const hash of index.matchAll(/href="#([^"]+)"/g)) {
  if (!ids.includes(hash[1] as string)) errors.push(`site/index.html: unresolved fragment #${hash[1]}`);
}

async function validateLocalReferences(path: string, content: string): Promise<void> {
  for (const match of content.matchAll(/(?:href|src)="([^"]+)"/g)) {
    const reference = match[1] as string;
    if (/^(?:https?:|mailto:|#|\/)/.test(reference)) continue;
    const cleanReference = reference.split("#")[0]?.split("?")[0];
    if (!cleanReference) continue;
    const target = resolve(root, dirname(path), cleanReference);
    if (!(await Bun.file(target).exists())) errors.push(`${path}: missing local reference ${reference}`);
  }
}

await validateLocalReferences("site/index.html", index);
await validateLocalReferences("site/404.html", notFound);

const structuredDataMatch = index.match(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/);
if (!structuredDataMatch?.[1]) {
  errors.push("site/index.html: missing JSON-LD structured data");
} else {
  try {
    const structuredData = JSON.parse(structuredDataMatch[1]) as { "@graph"?: unknown[] };
    if (!Array.isArray(structuredData["@graph"]) || structuredData["@graph"].length < 2) {
      errors.push("site/index.html: JSON-LD must describe the website and software");
    }
  } catch {
    errors.push("site/index.html: JSON-LD is not valid JSON");
  }
}

let manifest: {
  name?: string;
  start_url?: string;
  scope?: string;
  icons?: Array<{ src?: string; type?: string }>;
} = {};
try {
  manifest = JSON.parse(manifestText) as typeof manifest;
} catch {
  errors.push("site/site.webmanifest: manifest is not valid JSON");
}
if (
  manifest.name !== "Rebinder"
  || manifest.start_url !== "/rebinder/"
  || manifest.scope !== "/rebinder/"
  || manifest.icons?.[0]?.src !== "favicon.svg"
) {
  errors.push("site/site.webmanifest: canonical project scope or icon is invalid");
}

if (!robots.includes(`Sitemap: ${canonicalUrl}sitemap.xml`)) {
  errors.push("site/robots.txt: missing canonical sitemap URL");
}
if (!sitemap.includes(`<loc>${canonicalUrl}</loc>`)) {
  errors.push("site/sitemap.xml: missing canonical product URL");
}
if (!/^# Rebinder\n\n>\s+\S+/m.test(llms)) {
  errors.push("site/llms.txt: expected the project heading and summary blockquote");
}

for (const requiredCopy of [
  "rebinder sessions claude",
  "rebinder sessions codex",
  "rebinder transfer --from claude --to codex",
  "rebinder transfer --from codex --to claude",
  "both directions live",
  "private bounded checkpoint",
  "0.YYYYMMDD.REVISION",
  "Interchange Format 0.1.0",
]) {
  if (!index.includes(requiredCopy)) errors.push(`site/index.html: missing product truth ${requiredCopy}`);
}

if (!llms.includes("Two-way transfer is operational")) {
  errors.push("site/llms.txt: transfer availability boundary is missing");
}
if (styles.length < 10_000) errors.push("site/styles.css: product stylesheet appears incomplete");
if (!script.includes("prefers-reduced-motion")) errors.push("site/script.js: reduced-motion behavior is missing");
if (!notFound.includes('href="/rebinder/"')) errors.push("site/404.html: canonical recovery path is missing");

for (const installer of ["site/install.sh", "site/install.ps1"]) {
  const content = await readFile(installer, "utf8");
  if (!content.includes("bahadirarda/rebinder") || !content.includes("SHA256SUMS")) {
    errors.push(`${installer}: canonical repository or checksum verification is missing`);
  }
}

if (errors.length > 0) {
  for (const error of errors) console.error(`website validation: ${error}`);
  process.exit(1);
}

console.log("Validated the static website, metadata, discovery files, and installer boundary.");
