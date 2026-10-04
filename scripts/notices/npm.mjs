// Licenses of the JavaScript that ends up in the app: the production dependencies, and the
// Svelte runtime, a dev dependency compiled into the bundle.
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const app = process.argv[2];
const manifest = JSON.parse(readFileSync(join(app, "package.json"), "utf8"));
const names = [...Object.keys(manifest.dependencies ?? {}), "svelte"].sort();

let out = "## JavaScript packages\n\n";
for (const name of names) {
  const dir = join(app, "node_modules", name);
  const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
  out += `### ${name} ${pkg.version}\n\nLicense: ${pkg.license}\n\n`;
  for (const file of readdirSync(dir).filter((f) => /^licen[cs]e/i.test(f)).sort()) {
    out += "```text\n" + readFileSync(join(dir, file), "utf8").trim() + "\n```\n\n";
  }
}
process.stdout.write(out);
