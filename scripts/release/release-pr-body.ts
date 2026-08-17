import { writeFile } from "node:fs/promises";
import { isCalendarVersion } from "./calendar-version.ts";

function option(name: string): string {
  const index = Bun.argv.indexOf(name);
  const value = index >= 0 ? Bun.argv[index + 1] : undefined;
  if (!value) throw new Error(`${name} requires a value`);
  return value;
}

async function gitFile(ref: string, path: string): Promise<string> {
  const child = Bun.spawn(["git", "show", `${ref}:${path}`], {
    stderr: "inherit",
    stdout: "pipe",
  });
  const content = await new Response(child.stdout).text();
  if ((await child.exited) !== 0) throw new Error(`Cannot read ${path} from ${ref}`);
  return content;
}

const ref = option("--ref");
const output = option("--output");
if (!/^[A-Za-z0-9._/-]+$/.test(ref)) throw new Error(`Invalid Git reference ${ref}`);

const cargo = Bun.TOML.parse(await gitFile(ref, "Cargo.toml")) as {
  package?: { version?: string };
};
const version = cargo.package?.version;
if (!version || !isCalendarVersion(version)) {
  throw new Error(`${ref} does not contain a Rebinder calendar version`);
}
const changelog = await gitFile(ref, "CHANGELOG.md");
const escaped = version.replaceAll(".", "\\.");
const notes = changelog.match(
  new RegExp(`^## \\[${escaped}\\] - \\d{4}-\\d{2}-\\d{2}\\n([\\s\\S]*?)(?=^## \\[)`, "m"),
)?.[1]?.trim();
if (!notes) throw new Error(`CHANGELOG.md does not contain ${version}`);

await writeFile(
  output,
  `# Rebinder ${version}\n\nThis automated release pull request consumes reviewed Changesets and synchronizes the repository around one calendar release identity.\n\n## Release notes\n\n${notes}\n\n## Release identity\n\n- Canonical version: \`${version}\`\n- Annotated tag after merge: \`v${version}\`\n- Distribution: five native archives, verified installers, SHA-256 manifest, and GitHub provenance attestations\n- Required gates: Rust, release metadata, installer, package, and documentation validation\n`,
);
console.log(version);
