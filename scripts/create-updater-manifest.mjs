import { basename } from "node:path";
import { readFile, writeFile } from "node:fs/promises";

const [tag, platform, archivePath, signaturePath, notesPath, outputPath] = process.argv.slice(2);
if (!tag || !platform || !archivePath || !signaturePath || !notesPath || !outputPath) {
  throw new Error("usage: create-updater-manifest.mjs vVERSION PLATFORM ARCHIVE SIGNATURE NOTES OUTPUT");
}
const version = tag.startsWith("v") ? tag.slice(1) : tag;
if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
  throw new Error(`invalid release version: ${tag}`);
}
const signature = (await readFile(signaturePath, "utf8")).trim();
if (!signature) throw new Error("updater signature is empty");
const notes = (await readFile(notesPath, "utf8")).trim();
const encodedTag = encodeURIComponent(tag);
const encodedArchive = encodeURIComponent(basename(archivePath));
const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    [platform]: {
      signature,
      url: `https://github.com/kabudu/worth-weave/releases/download/${encodedTag}/${encodedArchive}`,
    },
  },
};
await writeFile(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, { flag: "wx" });
