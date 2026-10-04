// Stamps a release version into the manifests before a build: the tag is the source of
// truth, the files in the repository keep whatever they had. Usage: set-version.mjs v0.9.0
import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2]?.replace(/^v/, "");
if (!/^\d+\.\d+\.\d+([-+][0-9A-Za-z.-]+)?$/.test(version ?? "")) {
  console.error(`not a version: ${process.argv[2]}`);
  process.exit(1);
}
const edit = (file, change) => {
  const before = readFileSync(file, "utf8");
  const after = change(before);
  if (after === before && !before.includes(`"${version}"`)) {
    console.error(`no version to replace in ${file}`);
    process.exit(1);
  }
  writeFileSync(file, after);
};
edit("Cargo.toml", (s) => s.replace(/(\[workspace\.package\][^[]*?\nversion = )"[^"]*"/, `$1"${version}"`));
for (const file of ["app/src-tauri/tauri.conf.json", "app/package.json"]) {
  edit(file, (s) => s.replace(/("version": )"[^"]*"/, `$1"${version}"`));
}
console.log(version);
