// 화면 글 가운데 영어 표(crates/app/ui/lib/i18n-en.js)에 없는 것을 찾는다.
// 사용: node scripts/check-i18n.mjs   (빠진 글이 있으면 찍고 1 로 끝난다)
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { EN } from "../crates/app/ui/lib/i18n-en.js";

const root = new URL("../crates/app/ui/", import.meta.url).pathname;
const files = [
  ...readdirSync(root).filter((name) => name.endsWith(".js")),
  ...readdirSync(join(root, "lib")).filter((name) => name.endsWith(".js")).map((name) => `lib/${name}`),
].filter((name) => !name.startsWith("lib/i18n"));

const missing = new Map();
const used = new Set();
for (const file of files) {
  const source = readFileSync(join(root, file), "utf8");
  for (const match of source.matchAll(/\bt\(("(?:[^"\\]|\\.)*")/g)) {
    const text = JSON.parse(match[1]);
    const key = text.includes("\n") ? text.replace(/\s*\n\s*/g, " ") : text;
    used.add(key);
    if (/[가-힣]/.test(key) && !(key in EN)) {
      missing.set(key, file);
    }
  }
}
for (const [key, file] of missing) {
  console.log(`없음 (${file}): ${key}`);
}
const unused = Object.keys(EN).filter((key) => !used.has(key));
for (const key of unused) {
  console.log(`쓰지 않음: ${key}`);
}
console.log(`화면 글 ${used.size}개 · 영어 표 ${Object.keys(EN).length}개 · 빠진 글 ${missing.size}개 · 쓰지 않는 글 ${unused.length}개`);
process.exit(missing.size > 0 ? 1 : 0);
