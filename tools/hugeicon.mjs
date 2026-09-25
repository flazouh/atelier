// Converts one Hugeicons icon (`@hugeicons/core-free-icons`, MIT) to an SVG file.
// Usage: node tools/hugeicon.mjs <package>/dist/esm/<Name>Icon.js crates/beui/assets/icons/<name>.svg
import { writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const [, , src, dest] = process.argv;
const icon = (await import(pathToFileURL(src))).default;
const attrs = (a) =>
  Object.entries(a)
    .filter(([k]) => k !== "key")
    .map(([k, v]) => `${k.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase())}="${v}"`)
    .join(" ");
const body = icon.map(([tag, a]) => `<${tag} ${attrs(a)}/>`).join("");
writeFileSync(dest, `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none">${body}</svg>\n`);
