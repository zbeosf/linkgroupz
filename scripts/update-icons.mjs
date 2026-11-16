// 화면에서 쓰는 아이콘을 Phosphor Icons(MIT)에서 받아 crates/app/ui/lib/icon-data.js 로 만든다.
// 실행: node scripts/update-icons.mjs   (아이콘을 더하거나 뺄 때만 다시 돌린다)
import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const VERSION = "2.1.1";
const BASE = `https://unpkg.com/@phosphor-icons/core@${VERSION}/assets`;

// 그룹 아이콘: data.json 에 적는 이름(Swift 판과 같은 SF Symbol 이름) → Phosphor 아이콘
const GROUP_SYMBOLS = {
  "folder.fill": "folder-fill",
  "chevron.left.forwardslash.chevron.right": "code-bold",
  "curlybraces": "brackets-curly-bold",
  "terminal.fill": "terminal-window-fill",
  "arrow.triangle.branch": "git-branch-bold",
  "ladybug.fill": "bug-fill",
  "hammer.fill": "hammer-fill",
  "wrench.and.screwdriver.fill": "wrench-fill",
  "display": "monitor-bold",
  "server.rack": "hard-drives-fill",
  "cylinder.split.1x2.fill": "database-fill",
  "externaldrive.fill": "hard-drive-fill",
  "cpu": "cpu-bold",
  "network": "network-bold",
  "cloud.fill": "cloud-fill",
  "shippingbox.fill": "package-fill",
  "chart.xyaxis.line": "chart-line-bold",
  "chart.bar.fill": "chart-bar-fill",
  "chart.pie.fill": "chart-pie-fill",
  "doc.text.fill": "file-text-fill",
  "checklist": "list-checks-bold",
  "list.bullet.clipboard.fill": "clipboard-text-fill",
  "calendar": "calendar-blank-bold",
  "briefcase.fill": "briefcase-fill",
  "envelope.fill": "envelope-fill",
  "bubble.left.fill": "chat-circle-fill",
  "person.2.fill": "users-fill",
  "gearshape.fill": "gear-fill",
  "lock.fill": "lock-fill",
  "key.fill": "key-fill",
  "exclamationmark.triangle.fill": "warning-fill",
  "flag.fill": "flag-fill",
  "newspaper.fill": "newspaper-fill",
  "book.fill": "book-fill",
  "graduationcap.fill": "graduation-cap-fill",
  "globe": "globe-bold",
  "house.fill": "house-fill",
  "gamecontroller.fill": "game-controller-fill",
  "star.fill": "star-fill",
  "heart.fill": "heart-fill",
};

// 화면 곳곳의 버튼 · 표시에 쓰는 아이콘
const UI_ICONS = [
  "magnifying-glass", "x-circle-fill", "x-circle", "x-bold", "caret-right-bold", "caret-down-bold", "caret-left-bold",
  "caret-double-up", "caret-double-down", "folder-plus", "squares-four", "gear", "question", "power",
  "archive-fill", "archive", "trash", "pencil-simple", "eye", "eye-fill", "plus-bold", "detective-fill",
  "globe", "check-bold", "check-square-fill", "minus-square-fill", "square", "arrow-clockwise",
  "arrow-square-out", "stack", "tray-arrow-down", "list-checks", "info", "warning", "warning-fill",
  "arrow-u-up-left", "link", "image", "sidebar-simple", "hourglass", "check-circle-fill", "arrows-out",
  "arrow-bend-down-right", "arrow-circle-down-fill", "arrow-right", "copy", "folder", "rows",
  "browsers", "arrow-square-out-bold", "magnifying-glass-bold", "push-pin", "push-pin-fill", "gear-six",
  "funnel", "arrows-clockwise",
];

function location(name) {
  for (const weight of ["bold", "fill", "light", "thin", "duotone"]) {
    if (name.endsWith(`-${weight}`)) {
      return `${weight}/${name}.svg`;
    }
  }
  return `regular/${name}.svg`;
}

async function body(name) {
  const response = await fetch(`${BASE}/${location(name)}`);
  if (!response.ok) {
    throw new Error(`${name}: HTTP ${response.status}`);
  }
  const svg = await response.text();
  const inner = svg.match(/<svg[^>]*>([\s\S]*)<\/svg>/)?.[1]?.trim();
  if (!inner || !/^(<path d="[^"<>]+"\s*\/?>(<\/path>)?)+$/.test(inner)) {
    throw new Error(`${name}: 예상한 모양(<path> 만)이 아닙니다`);
  }
  // 경로 데이터만 남긴다
  return [...inner.matchAll(/d="([^"]+)"/g)].map((match) => match[1]);
}

const names = [...new Set([...Object.values(GROUP_SYMBOLS), ...UI_ICONS])].sort();
const paths = {};
for (const name of names) {
  paths[name] = await body(name);
}

const out = fileURLToPath(new URL("../crates/app/ui/lib/icon-data.js", import.meta.url));
const lines = [
  "// scripts/update-icons.mjs 가 만든 파일. 직접 고치지 않는다.",
  `// 그림: Phosphor Icons ${VERSION} (MIT, https://phosphoricons.com) — vendor/README.md 참고. viewBox 는 모두 0 0 256 256.`,
  "",
  "/** 그룹 아이콘 이름(data.json 에 적는 SF Symbol 이름) → 아이콘 */",
  `export const GROUP_SYMBOLS = ${JSON.stringify(GROUP_SYMBOLS, null, 2)};`,
  "",
  "/** 아이콘 이름 → 경로 데이터(<path d>) 목록 */",
  `export const ICON_PATHS = ${JSON.stringify(paths, null, 2)};`,
  "",
];
await writeFile(out, lines.join("\n"));
console.log(`${names.length}개 아이콘 → ${out}`);
