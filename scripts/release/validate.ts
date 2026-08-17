import { readdir, readFile } from "node:fs/promises";
import { isCalendarVersion, parseCalendarVersion } from "./calendar-version.ts";

const errors: string[] = [];

const cargoText = await readFile("Cargo.toml", "utf8");
const cargo = Bun.TOML.parse(cargoText) as {
  package?: { name?: string; version?: string; repository?: string };
};
const packageMetadata = JSON.parse(await readFile("package.json", "utf8")) as {
  name?: string;
  version?: string;
  private?: boolean;
};
const cargoLock = Bun.TOML.parse(await readFile("Cargo.lock", "utf8")) as {
  package?: Array<{ name?: string; version?: string; source?: string }>;
};
const version = cargo.package?.version;

if (cargo.package?.name !== "rebinder") errors.push("Cargo package must be named rebinder");
if (!version || !isCalendarVersion(version)) {
  errors.push("Cargo version must use 0.YYYYMMDD.REVISION");
}
if (cargo.package?.repository !== "https://github.com/bahadirarda/rebinder") {
  errors.push("Cargo repository URL is not canonical");
}
if (
  packageMetadata.name !== "rebinder"
  || packageMetadata.private !== true
  || packageMetadata.version !== version
) {
  errors.push("package.json must be the synchronized private Rebinder release proxy");
}

const lockPackage = cargoLock.package?.find(
  (candidate) => candidate.name === "rebinder" && candidate.source === undefined,
);
if (lockPackage?.version !== version) {
  errors.push(`Cargo.lock expected Rebinder ${version ?? "<missing>"}`);
}

const changelog = await readFile("CHANGELOG.md", "utf8");
const unreleasedBody = changelog.match(/^## \[Unreleased\]\n([\s\S]*?)(?=^## \[)/m)?.[1]?.trim();
if (unreleasedBody) errors.push("CHANGELOG release notes must come from pending Changesets");
if (version && isCalendarVersion(version)) {
  const parsed = parseCalendarVersion(version);
  const date = parsed
    ? `${parsed.date.slice(0, 4)}-${parsed.date.slice(4, 6)}-${parsed.date.slice(6, 8)}`
    : "";
  if (!changelog.includes(`## [${version}] - ${date}`)) {
    errors.push(`CHANGELOG.md is missing dated release ${version}`);
  }
  if (!changelog.includes(`[Unreleased]: https://github.com/bahadirarda/rebinder/compare/v${version}...HEAD`)) {
    errors.push("CHANGELOG.md Unreleased link does not start at the canonical version");
  }
}

const releaseTag = Bun.env.RELEASE_TAG;
if (releaseTag && releaseTag !== `v${version}`) {
  errors.push(`Release tag ${releaseTag} does not match v${version}`);
}

const changesetsConfig = JSON.parse(await readFile(".changeset/config.json", "utf8")) as {
  privatePackages?: { version?: boolean; tag?: boolean };
};
if (
  changesetsConfig.privatePackages?.version !== true
  || changesetsConfig.privatePackages.tag !== false
) {
  errors.push("Changesets private release proxy policy is invalid");
}

for (const path of [
  ".github/workflows/ci.yml",
  ".github/workflows/changesets.yml",
  ".github/workflows/release.yml",
  "site/install.sh",
  "site/install.ps1",
  "docs/assets/rebinder-hero.png",
]) {
  if (!(await Bun.file(path).exists())) errors.push(`${path} is required`);
}

for (const entry of await readdir(".changeset")) {
  if (!entry.endsWith(".md") || entry === "README.md") continue;
  const content = await readFile(`.changeset/${entry}`, "utf8");
  if (!/^---\n"rebinder": (patch|minor|major)\n---\n[\s\S]+/m.test(content)) {
    errors.push(`.changeset/${entry} is not a valid Rebinder release intent`);
  }
}

if (errors.length > 0) {
  for (const error of errors) console.error(`release validation: ${error}`);
  process.exit(1);
}

console.log(`Release metadata is synchronized at ${version}.`);
