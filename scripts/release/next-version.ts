import { readFile } from "node:fs/promises";
import { nextCalendarVersion } from "./calendar-version.ts";

async function gitDate(): Promise<string> {
  const child = Bun.spawn(["git", "show", "-s", "--format=%cs", "HEAD"], {
    stderr: "ignore",
    stdout: "pipe",
  });
  const output = (await new Response(child.stdout).text()).trim();
  if ((await child.exited) === 0 && output) return output;
  return new Date().toISOString().slice(0, 10);
}

const cargo = Bun.TOML.parse(await readFile("Cargo.toml", "utf8")) as {
  package?: { version?: string };
};
const previousVersion = cargo.package?.version;
if (!previousVersion) throw new Error("Cargo.toml does not contain a canonical version");

const releaseDate = Bun.env.RELEASE_DATE ?? await gitDate();
console.log(nextCalendarVersion(previousVersion, releaseDate));
