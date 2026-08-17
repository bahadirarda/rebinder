import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { nextCalendarVersion } from "./calendar-version.ts";

interface ReleasePlan {
  changesets: Array<{ id: string; summary: string }>;
  releases: Array<{
    name: string;
    oldVersion: string;
    newVersion: string;
    type: "major" | "minor" | "patch";
  }>;
}

async function run(command: string[]): Promise<void> {
  const child = Bun.spawn(command, { stderr: "inherit", stdout: "inherit" });
  if ((await child.exited) !== 0) throw new Error(`${command.join(" ")} failed`);
}

async function output(command: string[]): Promise<string> {
  const child = Bun.spawn(command, { stderr: "inherit", stdout: "pipe" });
  const text = (await new Response(child.stdout).text()).trim();
  if ((await child.exited) !== 0) throw new Error(`${command.join(" ")} failed`);
  return text;
}

function replaceExactly(content: string, pattern: RegExp, replacement: string, path: string): string {
  const matches = content.match(new RegExp(pattern.source, `${pattern.flags.replace("g", "")}g`));
  if (matches?.length !== 1) throw new Error(`${path}: expected exactly one match`);
  return content.replace(pattern, replacement);
}

function markdownBullet(summary: string): string {
  return `- ${summary.trim().replaceAll("\n", "\n  ")}`;
}

function updateChangelog(
  content: string,
  previousVersion: string,
  nextVersion: string,
  releaseDate: string,
  summaries: string[],
): string {
  const marker = "## [Unreleased]\n\n";
  if (!content.includes(marker)) throw new Error("CHANGELOG.md: missing Unreleased section");
  if (content.includes(`## [${nextVersion}]`)) {
    throw new Error(`CHANGELOG.md: release ${nextVersion} already exists`);
  }
  const section = [
    `## [${nextVersion}] - ${releaseDate}`,
    "",
    "### Changed",
    "",
    ...summaries.map(markdownBullet),
  ].join("\n");
  let updated = content.replace(marker, `${marker}${section}\n\n`);
  updated = replaceExactly(
    updated,
    /^\[Unreleased\]: .+$/m,
    `[Unreleased]: https://github.com/bahadirarda/rebinder/compare/v${nextVersion}...HEAD\n[${nextVersion}]: https://github.com/bahadirarda/rebinder/compare/v${previousVersion}...v${nextVersion}`,
    "CHANGELOG.md",
  );
  return updated;
}

const cargoPath = "Cargo.toml";
const packagePath = "package.json";
const cargoBefore = await readFile(cargoPath, "utf8");
const cargo = Bun.TOML.parse(cargoBefore) as { package?: { version?: string } };
const previousVersion = cargo.package?.version;
if (!previousVersion) throw new Error("Cargo.toml: missing canonical package version");

const planDirectory = await mkdtemp(join(tmpdir(), "rebinder-release-plan-"));
const planPath = join(planDirectory, "plan.json");

try {
  await run(["bun", "x", "changeset", "status", "--output", planPath]);
  const plan = JSON.parse(await readFile(planPath, "utf8")) as ReleasePlan;
  const release = plan.releases.find((candidate) => candidate.name === "rebinder");
  if (!release || plan.changesets.length === 0) {
    throw new Error("No pending Rebinder Changesets are available");
  }

  const releaseDate = Bun.env.RELEASE_DATE
    ?? await output(["git", "show", "-s", "--format=%cs", "HEAD"]);
  const nextVersion = nextCalendarVersion(previousVersion, releaseDate);
  const summaries = [...plan.changesets]
    .sort((left, right) => left.id.localeCompare(right.id))
    .map((changeset) => changeset.summary.trim());

  await run(["bun", "x", "changeset", "version"]);

  const packageMetadata = JSON.parse(await readFile(packagePath, "utf8")) as {
    name: string;
    version: string;
    [key: string]: unknown;
  };
  if (packageMetadata.version !== release.newVersion) {
    throw new Error(
      `package.json: Changesets produced ${packageMetadata.version}, expected ${release.newVersion}`,
    );
  }
  packageMetadata.version = nextVersion;
  await writeFile(packagePath, `${JSON.stringify(packageMetadata, null, 2)}\n`);

  const cargoAfter = replaceExactly(
    cargoBefore,
    /(\[package\][\s\S]*?\nversion = ")[^"]+("\n)/,
    `$1${nextVersion}$2`,
    cargoPath,
  );
  await writeFile(cargoPath, cargoAfter);

  const changelog = updateChangelog(
    await readFile("CHANGELOG.md", "utf8"),
    previousVersion,
    nextVersion,
    releaseDate,
    summaries,
  );
  await writeFile("CHANGELOG.md", changelog);

  await run(["bun", "install", "--lockfile-only"]);
  await run(["cargo", "check"]);
  await run(["bun", "run", "version:check"]);
  console.log(`Prepared Rebinder ${nextVersion} from ${plan.changesets.length} Changeset(s).`);
} finally {
  await rm(planDirectory, { force: true, recursive: true });
}
