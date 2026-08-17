import { readdir, readFile, stat } from "node:fs/promises";
import { dirname, extname, resolve } from "node:path";

const root = process.cwd();
const errors: string[] = [];

async function markdownFiles(directory: string): Promise<string[]> {
  const files: string[] = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if ([".git", "node_modules", "target"].includes(entry.name)) continue;
    const path = directory === "." ? entry.name : `${directory}/${entry.name}`;
    if (entry.isDirectory()) files.push(...await markdownFiles(path));
    else if (entry.isFile() && extname(entry.name) === ".md") files.push(path);
  }
  return files;
}

function frontmatter(content: string): string | null {
  if (!content.startsWith("---\n")) return null;
  const end = content.indexOf("\n---\n", 4);
  return end < 0 ? null : content.slice(4, end);
}

const files = await markdownFiles(".");
for (const path of files) {
  const content = await readFile(path, "utf8");
  if (path.startsWith("docs/")) {
    const metadata = frontmatter(content);
    if (!metadata) {
      errors.push(`${path}: missing YAML frontmatter`);
    } else {
      for (const key of ["type", "title", "status", "version"]) {
        if (!new RegExp(`^${key}:\\s*\\S+`, "m").test(metadata)) {
          errors.push(`${path}: missing frontmatter field ${key}`);
        }
      }
      const status = metadata.match(/^status:\s*(\S+)/m)?.[1];
      if (status && !["draft", "stable", "deprecated", "accepted"].includes(status)) {
        errors.push(`${path}: unsupported status ${status}`);
      }
    }
  }

  for (const match of content.matchAll(/!?\[[^\]]*\]\(([^)]+)\)/g)) {
    const raw = match[1]?.trim().replace(/^<|>$/g, "");
    if (!raw || /^(https?:|mailto:|#)/.test(raw)) continue;
    const target = raw.split("#")[0];
    if (!target) continue;
    const absolute = resolve(root, dirname(path), target);
    const exists = await stat(absolute).then(() => true, () => false);
    if (!exists) {
      errors.push(`${path}: broken Markdown link ${raw}`);
    }
  }
}

const rootIndex = await readFile("index.md", "utf8");
if (!rootIndex.startsWith("---\nokf_version: 0.2\n---\n")) {
  errors.push("index.md: expected the reserved OKF 0.2 frontmatter");
}
const log = await readFile("log.md", "utf8");
if (log.startsWith("---")) errors.push("log.md: reserved log must not have frontmatter");

if (errors.length > 0) {
  for (const error of errors) console.error(`documentation validation: ${error}`);
  process.exit(1);
}

console.log(`Validated ${files.length} Markdown documents.`);
