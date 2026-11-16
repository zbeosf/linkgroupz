// 가짜 앱. 화면 파일을 일반 브라우저에서 열었을 때(window.__TAURI__ 가 없을 때)만 쓰인다.
//
// 화면 모양을 앱 없이 확인하기 위한 것이라, 저장 · 브라우저 실행 같은 일은 흉내만 낸다.
// 주소 뒤에 옵션을 붙일 수 있다: ?lang=en&platform=windows&scene=archive&empty=1&update=available(downloading · installing)&extension=0

const options = new URLSearchParams(location.search);

// ?lang=en: 예시 데이터(그룹 · 탭 이름 등)도 영어로 (사용법 영어판 그림용)
const SAMPLE_EN = {
  "회색": "Grey",
  "파랑": "Blue",
  "빨강": "Red",
  "노랑": "Yellow",
  "초록": "Green",
  "분홍": "Pink",
  "보라": "Purple",
  "청록": "Cyan",
  "주황": "Orange",
  "남색": "Navy",
  "인디고": "Indigo",
  "틸": "Teal",
  "민트": "Mint",
  "라임": "Lime",
  "금색": "Gold",
  "코랄": "Coral",
  "갈색": "Brown",
  "자홍": "Magenta",
  "검정": "Black",
  "업무": "Work",
  "지라 보드": "Jira Board",
  "팀 위키": "Team Wiki",
  "대시보드": "Dashboard",
  "배포 · 모니터링": "Deploy · Monitoring",
  "젠킨스 배포": "Jenkins Deploy",
  "키바나": "Kibana",
  "읽을거리": "Reading",
  "디자인": "Design",
  "피그마": "Figma",
  "빈 그룹": "Empty Group",
  "회사": "Company",
  "지난 프로젝트": "Past Project",
  "기획 문서": "Planning Doc",
  "디자인 시안": "Design Draft",
  "기본": "Default",
  "개인": "Personal",
  "뉴스": "News",
  "취미": "Hobbies",
  "스프린트 42": "Sprint 42",
  "PR #556 — 탭 가져오기": "PR #556 — Import tabs",
  "확장 프로그램": "Extensions",
  "Rust 문서": "Rust Docs",
  "메일": "Mail",
  "압축해제된 확장 프로그램 로드": "Load unpacked",
  "압축을 푼 확장 로드": "Load unpacked",
  "압축해제 된 확장앱 설치": "Load unpacked extension",
  "자동 (확장이 있으면 탭 그룹)": "Auto (tab group with the extension)",
  "항상 탭 그룹": "Always a tab group",
  "항상 새 창": "Always a new window",
  "지난주 조사": "Last Week's Research",
  "Rust 공식 문서": "Rust Official Docs",
  "여행": "Travel",
  "항공권": "Flights",
  "이름 없는 그룹": "Untitled group",
  "지라 보드 — 스프린트 42": "Jira Board — Sprint 42",
  "상태 페이지": "Status Page",
  "런북": "Runbook"
};
const L = (text) => (options.get("lang") === "en" ? SAMPLE_EN[text] ?? text : text);
const platform = options.get("platform") ?? "macos";

const COLORS = [
  ["grey", L("회색"), [94, 99, 105]],
  ["blue", L("파랑"), [26, 115, 232]],
  ["red", L("빨강"), [217, 48, 38]],
  ["yellow", L("노랑"), [250, 171, 0]],
  ["green", L("초록"), [23, 128, 56]],
  ["pink", L("분홍"), [209, 23, 135]],
  ["purple", L("보라"), [161, 66, 240]],
  ["cyan", L("청록"), [0, 133, 143]],
  ["orange", L("주황"), [250, 125, 0]],
  ["navy", L("남색"), [28, 56, 135], "blue"],
  ["indigo", L("인디고"), [89, 87, 214], "purple"],
  ["teal", L("틸"), [48, 176, 199], "cyan"],
  ["mint", L("민트"), [0, 199, 166], "cyan"],
  ["lime", L("라임"), [140, 194, 26], "green"],
  ["gold", L("금색"), [204, 158, 33], "yellow"],
  ["coral", L("코랄"), [255, 115, 102], "red"],
  ["brown", L("갈색"), [140, 94, 56], "orange"],
  ["magenta", L("자홍"), [219, 56, 204], "pink"],
  ["black", L("검정"), [33, 33, 38], "grey"],
];
const colorName = (id) => COLORS.find((color) => color[0] === id)[1];

const PRESET_SYMBOLS = [
  "folder.fill", "chevron.left.forwardslash.chevron.right", "curlybraces", "terminal.fill", "arrow.triangle.branch", "ladybug.fill",
  "hammer.fill", "wrench.and.screwdriver.fill", "display", "server.rack", "cylinder.split.1x2.fill", "externaldrive.fill", "cpu",
  "network", "cloud.fill", "shippingbox.fill", "chart.xyaxis.line", "chart.bar.fill", "chart.pie.fill", "doc.text.fill", "checklist",
  "list.bullet.clipboard.fill", "calendar", "briefcase.fill", "envelope.fill", "bubble.left.fill", "person.2.fill", "gearshape.fill",
  "lock.fill", "key.fill", "exclamationmark.triangle.fill", "flag.fill", "newspaper.fill", "book.fill", "graduationcap.fill", "globe",
  "house.fill", "gamecontroller.fill", "star.fill", "heart.fill",
];

let counter = 0;
const newID = () => `00000000-0000-4000-8000-${String(++counter).padStart(12, "0")}`;
const link = (url, alias) => ({ id: newID(), url, ...(alias ? { alias } : {}) });
const group = (name, color, symbol, tags, links, extra = {}) => ({
  id: newID(),
  name,
  color,
  tags,
  links,
  browser: "chrome",
  icon: { symbol },
  alwaysPrivate: false,
  ...extra,
});

const store = {
  groups: options.has("empty")
    ? []
    : [
        group(L("업무"), "blue", "briefcase.fill", ["work", "daily"], [
          link("https://jira.example.com/secure/RapidBoard.jspa?rapidView=12", L("지라 보드")),
          link("https://wiki.example.com/display/TEAM/Home", L("팀 위키")),
          link("https://bitbucket.example.com/projects/APP"),
          link("https://grafana.example.com/d/abc/overview", L("대시보드")),
          link("https://calendar.example.com"),
        ]),
        group(L("배포 · 모니터링"), "navy", "server.rack", ["ops"], [
          link("https://jenkins.example.com/job/deploy", L("젠킨스 배포")),
          link("https://kibana.example.com/app/discover", L("키바나")),
        ], { alwaysPrivate: true }),
        group(L("읽을거리"), "coral", "newspaper.fill", ["read"], [
          link("https://news.ycombinator.com", "Hacker News"),
          link("https://www.reddit.com/r/rust"),
          link("https://this-week-in-rust.org", "This Week in Rust"),
        ], { browser: "edge" }),
        group(L("디자인"), "magenta", "star.fill", [], [link("https://www.figma.com/files/recent", L("피그마"))]),
        group(L("빈 그룹"), "grey", "folder.fill", [], []),
      ],
  archive: { groups: [], links: [] },
  sections: [],
  sidebarOrder: [],
  lastError: options.get("error") ?? null,
};
// 섹션 예시 (?sections=0 이면 없음)
if (!options.has("empty") && options.get("sections") !== "0") {
  const section = { id: newID(), name: L("회사"), isCollapsed: false };
  store.sections.push(section);
  store.groups[0].sectionID = section.id;
  store.groups[1].sectionID = section.id;
  store.sidebarOrder = [`section:${section.id}`, ...store.groups.slice(2).map((item) => `group:${item.id}`)];
}
const now = Date.now() / 1000 - 978307200;
if (!options.has("empty")) {
  store.archive.groups.push({
    group: group(L("지난 프로젝트"), "teal", "flag.fill", ["2025"], [link("https://www.notion.so/plan", L("기획 문서")), link("https://www.figma.com/file/x", L("디자인 시안"))]),
    archivedAt: now - 86400 * 12,
  });
  store.archive.links.push({
    link: link("https://www.apple.com/kr/", "Apple"),
    groupID: store.groups[2].id,
    groupName: L("읽을거리"),
    groupColor: "coral",
    archivedAt: now - 86400 * 3,
  });
}

const listeners = new Map();
function emit(event, payload) {
  for (const handler of listeners.get(event) ?? []) {
    handler(structuredClone(payload));
  }
}
const allTags = () => [...new Set(store.groups.flatMap((item) => item.tags))].sort();
const snapshot = () => structuredClone({ ...store, sidebarOrder: sidebarOrder(), allTags: allTags() });
const changed = () => {
  normalizeSections();
  emit("store-changed", snapshot());
};

// 프로필: crates/app/src/profiles.rs 를 흉내 낸다. (?profiles=0 이면 기본 프로필 하나)
let searchScopeState = { name: true, tags: true, url: true };
const PROFILE_ICON = { symbol: "person.2.fill" };
const profileState = { current: "default", profiles: [{ id: "default", name: L("기본"), icon: PROFILE_ICON, color: "blue", isDefault: true }] };
const profileData = new Map();
if (options.get("profiles") !== "0" && !options.has("empty")) {
  profileState.profiles.push({ id: "p-personal", name: L("개인"), icon: { symbol: "house.fill" }, color: "green", isDefault: false });
  profileData.set("p-personal", {
    groups: [
      group(L("뉴스"), "red", "newspaper.fill", ["read"], [link("https://news.ycombinator.com", "Hacker News"), link("https://www.theverge.com", "The Verge")]),
      group(L("취미"), "green", "gamecontroller.fill", [], [link("https://www.youtube.com", "YouTube")]),
    ],
    archive: { groups: [], links: [] },
    sections: [],
    sidebarOrder: [],
  });
}
const profileGroups = (id) => (id === profileState.current ? store.groups : (profileData.get(id)?.groups ?? []));
const profileView = () => ({
  current: profileState.current,
  profiles: profileState.profiles.map((profile) => ({
    ...profile,
    groups: profileGroups(profile.id).length,
    links: profileGroups(profile.id).reduce((sum, item) => sum + item.links.length, 0),
  })),
});
const profilesChanged = () => emit("profiles-changed", profileView());
function switchProfile(id) {
  if (id === profileState.current || !profileState.profiles.some((profile) => profile.id === id)) {
    return;
  }
  profileData.set(profileState.current, structuredClone({ groups: store.groups, archive: store.archive, sections: store.sections, sidebarOrder: store.sidebarOrder }));
  Object.assign(store, structuredClone(profileData.get(id) ?? { groups: [], archive: { groups: [], links: [] }, sections: [], sidebarOrder: [] }));
  profileState.current = id;
  changed();
  profilesChanged();
}
const profileName = (name) => {
  const trimmed = name.trim().split(/\s+/).join(" ");
  if (!trimmed || [...trimmed].length > 30) {
    throw "이름을 1~30글자로 적어 주세요.";
  }
  return trimmed;
};
const sameProfileName = (name, except) => profileState.profiles.some((profile) => profile.id !== except && profile.name.toLowerCase() === name.toLowerCase());

// 섹션: crates/core/src/sections.rs 를 흉내 낸다.
const splitItem = (item) => [item.slice(0, item.indexOf(":")), item.slice(item.indexOf(":") + 1)];
const sidebarOrder = () => (store.sections.length ? store.sidebarOrder : store.groups.map((item) => `group:${item.id}`));
function normalizeSections() {
  const sectionIDs = new Set(store.sections.map((section) => section.id));
  for (const item of store.groups) {
    if (item.sectionID && !sectionIDs.has(item.sectionID)) {
      delete item.sectionID;
    }
  }
  if (store.sections.length === 0) {
    store.sidebarOrder = [];
    return;
  }
  const placed = new Set();
  const order = [];
  for (const entry of store.sidebarOrder) {
    const [kind, id] = splitItem(entry);
    const valid = kind === "section" ? sectionIDs.has(id) : store.groups.some((item) => item.id === id && !item.sectionID);
    if (valid && !placed.has(id)) {
      placed.add(id);
      order.push(entry);
    }
  }
  for (const item of store.groups) {
    const id = item.sectionID ?? item.id;
    if (!placed.has(id)) {
      placed.add(id);
      order.push(item.sectionID ? `section:${id}` : `group:${id}`);
    }
  }
  for (const section of store.sections) {
    if (!placed.has(section.id)) {
      placed.add(section.id);
      order.push(`section:${section.id}`);
    }
  }
  const groups = [];
  for (const entry of order) {
    const [kind, id] = splitItem(entry);
    groups.push(...store.groups.filter((item) => (kind === "group" ? item.id === id : item.sectionID === id)));
  }
  store.groups = groups;
  store.sidebarOrder = order;
}
function moveItems(ids, target) {
  const moving = new Set(ids);
  const into = target.section && store.sections.some((section) => section.id === target.section) && !moving.has(target.section) ? target.section : null;
  if (store.sections.length === 0 && !into) {
    const moved = store.groups.filter((item) => moving.has(item.id));
    const kept = store.groups.filter((item) => !moving.has(item.id));
    let index = kept.findIndex((item) => item.id === target.before);
    if (index < 0) {
      const after = kept.findIndex((item) => item.id === target.after);
      index = after >= 0 ? after + 1 : kept.length;
    }
    kept.splice(index, 0, ...moved);
    store.groups = kept;
    return;
  }
  normalizeSections();
  const ranks = new Map();
  let rank = 0;
  for (const entry of store.sidebarOrder) {
    const [kind, id] = splitItem(entry);
    ranks.set(id, rank++);
    if (kind === "section") {
      store.groups.filter((item) => item.sectionID === id).forEach((item) => ranks.set(item.id, rank++));
    }
  }
  const movingSections = store.sidebarOrder.filter((entry) => entry.startsWith("section:") && moving.has(splitItem(entry)[1]));
  const movingGroups = store.groups.filter((item) => moving.has(item.id));
  store.sidebarOrder = store.sidebarOrder.filter((entry) => !moving.has(splitItem(entry)[1]));
  if (into) {
    const kept = store.groups.filter((item) => !moving.has(item.id));
    movingGroups.forEach((item) => (item.sectionID = into));
    let index = kept.findIndex((item) => item.id === target.before && item.sectionID === into);
    if (index < 0) {
      const last = kept.map((item) => item.sectionID).lastIndexOf(into);
      index = last >= 0 ? last + 1 : kept.length;
    }
    kept.splice(index, 0, ...movingGroups);
    store.groups = kept;
    if (movingSections.length) {
      store.sidebarOrder.splice(Math.max(0, store.sidebarOrder.indexOf(`section:${into}`)), 0, ...movingSections);
    }
    return;
  }
  movingGroups.forEach((item) => delete item.sectionID);
  const entries = [...movingSections, ...movingGroups.map((item) => `group:${item.id}`)].sort((a, b) => ranks.get(splitItem(a)[1]) - ranks.get(splitItem(b)[1]));
  let index = store.sidebarOrder.findIndex((entry) => splitItem(entry)[1] === target.before);
  if (index < 0) {
    const after = store.sidebarOrder.findIndex((entry) => splitItem(entry)[1] === target.after);
    index = after >= 0 ? after + 1 : store.sidebarOrder.length;
  }
  store.sidebarOrder.splice(index, 0, ...entries);
}
const findGroup = (id) => store.groups.find((item) => item.id === id);

const fold = (text) => text.normalize("NFKD").replace(/\p{M}/gu, "").normalize("NFC").toLowerCase();
function filterGroup(item, query) {
  const terms = query.split(/\s+/).filter(Boolean).map(fold);
  const tagTerms = terms.filter((term) => term.startsWith("#")).map((term) => term.replace(/^#+/, "")).filter(Boolean);
  const textTerms = terms.filter((term) => !term.startsWith("#"));
  const tags = item.tags.map(fold);
  if (!tagTerms.every((term) => tags.some((tag) => tag.includes(term)))) {
    return null;
  }
  const groupText = `${fold(item.name)} ${tags.join(" ")}`;
  const remaining = textTerms.filter((term) => !groupText.includes(term));
  if (remaining.length === 0) {
    return item;
  }
  const links = item.links.filter((entry) => remaining.every((term) => fold(`${entry.alias ?? ""} ${entry.url}`).includes(term)));
  return links.length > 0 ? { ...item, links } : null;
}

function parseTags(text) {
  const seen = new Set();
  const result = [];
  for (const raw of text.split(/[,\s]+/)) {
    const tag = raw.trim().replace(/^#+/, "").trim();
    if (tag && !seen.has(tag.toLowerCase())) {
      seen.add(tag.toLowerCase());
      result.push(tag);
    }
  }
  return result;
}

function parseUrl(text) {
  const trimmed = text.trim();
  if (!trimmed || trimmed.includes(" ")) {
    return null;
  }
  try {
    const url = new URL(trimmed.includes("://") ? trimmed : `https://${trimmed}`);
    return ["http:", "https:", "file:"].includes(url.protocol) ? url.href : null;
  } catch {
    return null;
  }
}

const tabGroupInfo = { id: 7, title: L("스프린트 42"), color: "purple" };
const TABS = [
  { windowIndex: 1, title: L("지라 보드"), url: "https://jira.example.com/secure/RapidBoard.jspa?rapidView=12", tabGroup: tabGroupInfo },
  { windowIndex: 1, title: L("PR #556 — 탭 가져오기"), url: "https://bitbucket.example.com/projects/APP/pull-requests/556", tabGroup: tabGroupInfo },
  { windowIndex: 1, title: L("확장 프로그램"), url: "chrome://extensions/", tabGroup: tabGroupInfo },
  { windowIndex: 1, title: L("Rust 문서"), url: "https://doc.rust-lang.org/book/" },
  { windowIndex: 2, title: "Tauri", url: "https://tauri.app/start/", tabGroup: { id: 9, title: "", color: "green" } },
  { windowIndex: 2, title: L("메일"), url: "https://mail.example.com/inbox" },
];
function candidatesFor(tabs, existing) {
  const has = new Set(existing.map((entry) => entry.url.replace(/#.*$/, "").replace(/\/+$/, "").toLowerCase()));
  return tabs.map((tab, id) => {
    const supported = /^(https?|file):/.test(tab.url);
    const key = tab.url.replace(/#.*$/, "").replace(/\/+$/, "").toLowerCase();
    return { id, tab, url: supported ? tab.url : null, status: !supported ? "unsupported" : has.has(key) ? "alreadyInGroup" : "available" };
  });
}

const backupState = {
  days: 7,
  choices: [0, 3, 7, 14, 30, 4294967295],
  keepAll: 4294967295,
  backups: options.has("empty") ? [] : [{ date: "2026-11-12", groupCount: 5, size: 2048 }, { date: "2026-11-11", groupCount: 5, size: 2010 }, { date: "2026-11-09", groupCount: 4, size: 1800 }],
  folder: "~/Library/Application Support/LinkGroupz/backups",
};
let popoverShortcut = { shortcut: options.get("shortcut") ?? null, active: options.has("shortcut") };
const update = {
  configured: options.get("update") !== "none",
  autoCheck: options.get("update") !== "off",
  currentVersion: "1.0.0",
  availableVersion: options.get("update") === "available" ? "1.1.0" : null,
  lastChecked: options.get("update") ? Date.now() - 60000 : null,
  // ?update=downloading · installing: 업데이트 진행 창(update.html) 확인용
  phase: ["downloading", "installing"].includes(options.get("update")) ? options.get("update") : "idle",
  message: null,
  installingVersion: ["downloading", "installing"].includes(options.get("update")) ? "1.1.0" : null,
  downloaded: 4_404_019,
  total: 8_493_465,
};
const extensionFound =
  options.get("extension") === "0"
    ? null
    : {
        profile: "Default",
        extensionID: "abcdefghijklmnop",
        path: "/Users/me/Library/Application Support/LinkGroupz/chrome-extension",
      };
let launch = { state: "disabled", installedLocation: true };

const commands = {
  bootstrap: () => ({
    appName: "LinkGroupz",
    version: "1.0.0",
    profiles: profileView(),
    searchScope: searchScopeState,
    platform,
    colors: COLORS.map(([id, name, rgb, chrome]) => ({ id, name, rgb, chromeId: chrome ?? id, chromeName: colorName(chrome ?? id), isChrome: !chrome })),
    presetSymbols: PRESET_SYMBOLS,
    defaultSymbol: "folder.fill",
    browsers: [
      { id: "chrome", name: "Chrome", fullName: "Google Chrome", installed: true, extensionsPage: "chrome://extensions", loadUnpackedLabel: L("압축해제된 확장 프로그램 로드") },
      { id: "edge", name: "Edge", fullName: "Microsoft Edge", installed: options.get("edge") !== "0", extensionsPage: "edge://extensions", loadUnpackedLabel: L("압축을 푼 확장 로드") },
      { id: "whale", name: "Whale", fullName: "Naver Whale", installed: options.get("whale") !== "0", extensionsPage: "whale://extensions", loadUnpackedLabel: L("압축해제 된 확장앱 설치") },
    ],
    openModes: [
      { id: "auto", name: L("자동 (확장이 있으면 탭 그룹)") },
      { id: "tabGroup", name: L("항상 탭 그룹") },
      { id: "newWindow", name: L("항상 새 창") },
    ],
    groupOpenMode: "auto",
    dragPrefix: "linkgroupz-links:",
    showDockIcon: options.get("dock") !== "0",
    showManagerOnLaunch: options.get("managerOnLaunch") === "1",
    dataFile: platform === "windows" ? "C:\\Users\\me\\AppData\\Roaming\\LinkGroupz\\data.json" : "~/Library/Application Support/LinkGroupz/data.json",
    scene: options.get("scene"),
    // ?lang=en: 영어 화면
    language: options.get("lang") ?? "ko",
    languageSetting: options.get("lang") ?? null,
    themeSetting: options.get("theme") ?? null,
    hasGuide: true,
    hasLicenses: true,
  }),
  store_snapshot: snapshot,
  filter_groups: ({ query }) => store.groups.map((item) => filterGroup(item, query)).filter(Boolean),
  filter_archive: ({ query }) => ({
    groups: store.archive.groups.map((item) => ({ ...item, group: filterGroup(item.group, query) })).filter((item) => item.group),
    links: store.archive.links.filter((item) => filterGroup({ name: item.groupName, tags: [], links: [item.link] }, query)),
  }),
  parse_url: ({ text }) => parseUrl(text),
  parse_tags: ({ text }) => parseTags(text),
  add_group: ({ input }) => {
    const created = { id: newID(), links: [], ...input };
    store.groups.push(created);
    changed();
    return created.id;
  },
  update_group: ({ id, input }) => {
    Object.assign(findGroup(id), input);
    changed();
  },
  set_group_browser: ({ id, browser }) => {
    findGroup(id).browser = browser;
    changed();
  },
  set_group_always_private: ({ id, alwaysPrivate }) => {
    findGroup(id).alwaysPrivate = alwaysPrivate;
    changed();
  },
  delete_groups: ({ ids }) => {
    store.groups = store.groups.filter((item) => !ids.includes(item.id));
    changed();
  },
  move_items: ({ ids, target }) => {
    moveItems(ids, target);
    changed();
  },
  add_section: ({ name, at, groupIds }) => {
    if (store.sections.length === 0) {
      store.sidebarOrder = store.groups.map((item) => `group:${item.id}`);
    }
    normalizeSections();
    const section = { id: newID(), name: name.trim(), isCollapsed: false };
    const atID = findGroup(at)?.sectionID ?? at;
    const index = store.sidebarOrder.findIndex((entry) => splitItem(entry)[1] === atID);
    store.sections.push(section);
    store.sidebarOrder.splice(index >= 0 ? index : store.sidebarOrder.length, 0, `section:${section.id}`);
    if (groupIds.length) {
      moveItems(groupIds, { section: section.id });
    }
    changed();
    return section.id;
  },
  rename_section: ({ id, name }) => {
    store.sections.find((section) => section.id === id).name = name.trim();
    changed();
  },
  set_section_collapsed: ({ id, collapsed }) => {
    store.sections.find((section) => section.id === id).isCollapsed = collapsed;
    changed();
  },
  delete_section: ({ id }) => {
    const index = store.sidebarOrder.indexOf(`section:${id}`);
    const members = store.groups.filter((item) => item.sectionID === id);
    store.sidebarOrder.splice(index, 1, ...members.map((item) => `group:${item.id}`));
    members.forEach((item) => delete item.sectionID);
    store.sections = store.sections.filter((section) => section.id !== id);
    changed();
  },
  move_section: ({ id, offset }) => {
    const index = store.sidebarOrder.indexOf(`section:${id}`);
    const target = Math.max(0, Math.min(store.sidebarOrder.length - 1, index + offset));
    const [entry] = store.sidebarOrder.splice(index, 1);
    store.sidebarOrder.splice(target, 0, entry);
    changed();
  },
  load_closed_tab_groups: async () => {
    await new Promise((resolve) => setTimeout(resolve, 120));
    const info = (id, title, color) => ({ id, title, color, isClosed: true });
    const closed = [
      { info: info(-1, L("지난주 조사"), "purple"), windowIndex: 0, tabs: [{ windowIndex: 0, title: L("Rust 공식 문서"), url: "https://doc.rust-lang.org/book/", tabGroup: info(-1, L("지난주 조사"), "purple") }, { windowIndex: 0, title: "Tauri", url: "https://tauri.app", tabGroup: info(-1, L("지난주 조사"), "purple") }] },
      { info: info(-2, L("여행"), "orange"), windowIndex: 0, tabs: [{ windowIndex: 0, title: L("항공권"), url: "https://www.example.com/flights", tabGroup: info(-2, L("여행"), "orange") }] },
    ];
    return closed.map((tabGroup) => ({ tabGroup, displayTitle: tabGroup.info.title, candidates: candidatesFor(tabGroup.tabs, []), existing: null }));
  },
  set_show_dock_icon: () => null,
  set_show_manager_on_launch: () => null,
  backups: () => backupState,
  set_backup_days: ({ days }) => {
    backupState.days = days;
    return backupState;
  },
  restore_backup: () => null,
  export_data: () => "~/Downloads/LinkGroupz-2026-11-12.json",
  choose_import_file: () => ({ path: "/tmp/x.json", fileName: "LinkGroupz-2026-11-12.json", summary: { groups: 5, links: 11, sections: 1, archived: 2 } }),
  import_data: () => null,
  move_groups: ({ from, to }) => {
    const moving = from.map((index) => store.groups[index]);
    const before = from.filter((index) => index < to).length;
    store.groups = store.groups.filter((_, index) => !from.includes(index));
    store.groups.splice(to - before, 0, ...moving);
    changed();
  },
  archive_groups: ({ ids }) => {
    const moving = store.groups.filter((item) => ids.includes(item.id));
    store.groups = store.groups.filter((item) => !ids.includes(item.id));
    store.archive.groups.unshift(...moving.map((item) => ({ group: item, archivedAt: Date.now() / 1000 - 978307200 })));
    changed();
  },
  add_link: ({ groupId, url, alias }) => {
    const parsed = parseUrl(url);
    if (!parsed) {
      throw "http(s) URL 형식이 아닙니다.";
    }
    findGroup(groupId).links.push(link(parsed, alias));
    changed();
  },
  update_link: ({ groupId, linkId, url, alias }) => {
    Object.assign(findGroup(groupId).links.find((entry) => entry.id === linkId), { url: parseUrl(url), alias: alias ?? undefined });
    changed();
  },
  add_links: ({ groupId, links }) => {
    findGroup(groupId).links.push(...links.map((entry) => link(entry.url, entry.alias)));
    changed();
  },
  delete_links: ({ groupId, ids }) => {
    const target = findGroup(groupId);
    target.links = target.links.filter((entry) => !ids.includes(entry.id));
    changed();
  },
  move_links_within: ({ groupId, from, to }) => {
    const target = findGroup(groupId);
    const moving = from.map((index) => target.links[index]);
    const before = from.filter((index) => index < to).length;
    target.links = target.links.filter((_, index) => !from.includes(index));
    target.links.splice(to - before, 0, ...moving);
    changed();
  },
  move_links: ({ ids, sourceId, targetId }) => {
    const source = findGroup(sourceId);
    const moving = source.links.filter((entry) => ids.includes(entry.id));
    source.links = source.links.filter((entry) => !ids.includes(entry.id));
    findGroup(targetId).links.push(...moving);
    changed();
    return { moved: moving.length, duplicates: 0 };
  },
  archive_links: ({ groupId, ids }) => {
    const source = findGroup(groupId);
    const moving = source.links.filter((entry) => ids.includes(entry.id));
    source.links = source.links.filter((entry) => !ids.includes(entry.id));
    store.archive.links.unshift(
      ...moving.map((entry) => ({ link: entry, groupID: source.id, groupName: source.name, groupColor: source.color, archivedAt: Date.now() / 1000 - 978307200 })),
    );
    changed();
  },
  restore_groups: ({ ids }) => {
    const restoring = store.archive.groups.filter((item) => ids.includes(item.group.id));
    store.archive.groups = store.archive.groups.filter((item) => !ids.includes(item.group.id));
    store.groups.push(...restoring.map((item) => item.group));
    changed();
  },
  restore_links: ({ ids, targetId }) => {
    const restoring = store.archive.links.filter((item) => ids.includes(item.link.id));
    store.archive.links = store.archive.links.filter((item) => !ids.includes(item.link.id));
    findGroup(targetId).links.push(...restoring.map((item) => item.link));
    changed();
    return { moved: restoring.length, duplicates: 0 };
  },
  restore_links_to_new_group: ({ ids, input }) => {
    const groupID = commands.add_group({ input });
    return { groupID, result: commands.restore_links({ ids, targetId: groupID }) };
  },
  delete_archived_groups: ({ ids }) => {
    store.archive.groups = store.archive.groups.filter((item) => !ids.includes(item.group.id));
    changed();
  },
  // 보관함 끌어 놓기 (crates/core/src/archive.rs 를 흉내 낸다)
  restore_groups_to: ({ ids, target }) => {
    commands.restore_groups({ ids });
    moveItems(ids, target);
    changed();
  },
  restore_archived_group_links: ({ groupId, ids, targetId }) => {
    const source = store.archive.groups.find((item) => item.group.id === groupId)?.group;
    const moving = source.links.filter((item) => ids.includes(item.id));
    source.links = source.links.filter((item) => !ids.includes(item.id));
    findGroup(targetId).links.push(...moving);
    changed();
    return { moved: moving.length, duplicates: 0 };
  },
  delete_archived_group_links: ({ groupId, ids }) => {
    const source = store.archive.groups.find((item) => item.group.id === groupId)?.group;
    source.links = source.links.filter((item) => !ids.includes(item.id));
    changed();
  },
  move_archived_links_into_group: ({ ids, groupId }) => {
    const target = store.archive.groups.find((item) => item.group.id === groupId)?.group;
    const moving = store.archive.links.filter((item) => ids.includes(item.link.id));
    store.archive.links = store.archive.links.filter((item) => !ids.includes(item.link.id));
    target.links.push(...moving.map((item) => item.link));
    changed();
  },
  move_archived_groups: ({ from, to }) => {
    const list = store.archive.groups;
    const moving = from.map((index) => list[index]);
    const before = from.filter((index) => index < to).length;
    store.archive.groups = list.filter((_, index) => !from.includes(index));
    store.archive.groups.splice(to - before, 0, ...moving);
    changed();
  },
  move_archived_links: ({ from, to }) => {
    const list = store.archive.links;
    const moving = from.map((index) => list[index]);
    const before = from.filter((index) => index < to).length;
    store.archive.links = list.filter((_, index) => !from.includes(index));
    store.archive.links.splice(to - before, 0, ...moving);
    changed();
  },
  move_links_in_archived_group: ({ groupId, from, to }) => {
    const target = store.archive.groups.find((item) => item.group.id === groupId)?.group;
    const moving = from.map((index) => target.links[index]);
    const before = from.filter((index) => index < to).length;
    target.links = target.links.filter((_, index) => !from.includes(index));
    target.links.splice(to - before, 0, ...moving);
    changed();
  },
  archive_section: ({ id }) => {
    commands.archive_groups({ ids: store.groups.filter((item) => item.sectionID === id).map((item) => item.id) });
    commands.delete_section({ id });
  },
  delete_archived_links: ({ ids }) => {
    store.archive.links = store.archive.links.filter((item) => !ids.includes(item.link.id));
    changed();
  },
  browser_states: () => [
    { id: "chrome", installed: true, running: true },
    { id: "edge", installed: options.get("edge") !== "0", running: false },
    { id: "whale", installed: options.get("whale") !== "0", running: options.get("whale") === "running" },
  ],
  load_tabs: async ({ groupId }) => {
    await new Promise((resolve) => setTimeout(resolve, 150));
    if (options.get("tabs") === "error") {
      throw "실행 중인 Chrome 창이 없습니다. Chrome 창을 연 뒤 다시 시도해 주세요.";
    }
    return {
      candidates: candidatesFor(TABS, findGroup(groupId)?.links ?? []),
      note: extensionFound ? null : "Chrome 에 LinkGroupz 확장이 없어 탭 그룹 없이 표시합니다.",
    };
  },
  load_tab_groups: async () => {
    await new Promise((resolve) => setTimeout(resolve, 150));
    if (!extensionFound) {
      throw "Chrome 탭 그룹은 Chrome 에 LinkGroupz 확장이 있어야 읽을 수 있습니다. 설정의 ‘확장 설치’를 참고해 주세요.";
    }
    const groups = new Map();
    for (const tab of TABS.filter((item) => item.tabGroup)) {
      const entry = groups.get(tab.tabGroup.id) ?? { info: tab.tabGroup, windowIndex: tab.windowIndex, tabs: [] };
      entry.tabs.push(tab);
      groups.set(tab.tabGroup.id, entry);
    }
    return [...groups.values()].map((tabGroup, index) => {
      const existing = index === 1 ? store.groups[0] : null;
      return {
        tabGroup,
        displayTitle: tabGroup.info.title || L("이름 없는 그룹"),
        candidates: candidatesFor(tabGroup.tabs, existing?.links ?? []),
        existing: existing ? { id: existing.id, name: existing.name, color: existing.color } : null,
      };
    });
  },
  plan_tab_group_import: ({ selection }) => {
    const chosen = Object.entries(selection.selections).filter(([, ids]) => ids.length > 0);
    return { newGroups: chosen.length, additions: 0, pageCount: chosen.reduce((sum, [, ids]) => sum + ids.length, 0) };
  },
  import_tab_groups: () => null,
  extension_states: () => [
    { browser: "chrome", installed: true, found: extensionFound },
    { browser: "edge", installed: options.get("edge") !== "0", found: null },
    { browser: "whale", installed: options.get("whale") !== "0", found: null },
  ],
  extension_folder: () =>
    platform === "windows" ? "C:\\Users\\me\\AppData\\Roaming\\LinkGroupz\\chrome-extension" : "~/Library/Application Support/LinkGroupz/chrome-extension",
  popover_layout: () => ({ fixed: options.has("fixed"), detached: options.has("detached"), anchoredAbove: options.get("above") === "1" }),
  set_popover_detached: () => null,
  set_popover_size: () => null,
  popover_shortcut: () => popoverShortcut,
  set_popover_shortcut: ({ shortcut }) => {
    if (shortcut && !/^(Control|Alt|Super)\+|\+(Control|Alt|Super)\+|^F\d+$/.test(shortcut)) {
      throw "Ctrl · Alt · ⌘(Windows 키) 중 하나와 함께 눌러 주세요.";
    }
    popoverShortcut = { shortcut: shortcut ?? null, active: Boolean(shortcut) };
    return popoverShortcut;
  },
  pause_popover_shortcut: () => null,
  launch_at_login: () => launch,
  set_launch_at_login: ({ enabled }) => (launch = { ...launch, state: enabled ? "enabled" : "disabled" }),
  toggle_preview: () => true,
  previewing_link: () => null,
  preview_info: () => ({
    label: "preview",
    compact: options.get("mode") !== "expanded",
    title: L("지라 보드 — 스프린트 42"),
    fallbackTitle: L("지라 보드"),
    url: "https://jira.example.com/secure/RapidBoard.jspa?rapidView=12",
    loading: options.has("loading"),
    slow: options.has("slow"),
    browser: "chrome",
    isPrivate: false,
    linkID: null,
  }),
  preview_site_count: () => 3,
  update_status: () => update,
  update_check: () => {
    update.message = "최신 버전입니다.";
    emit("update-changed", update);
  },
  update_clear_message: () => {
    update.message = null;
    emit("update-changed", update);
  },
  update_set_auto_check: ({ enabled }) => {
    update.autoCheck = enabled;
    emit("update-changed", update);
  },
  // 그룹 동기화: 첫 페이지는 같고, 브라우저에만 있는 페이지 2개 (?sync=none 이면 탭 그룹 없음)
  sync_group_plan: ({ id }) => {
    const group = store.groups.find((item) => item.id === id);
    if (options.get("sync") === "none") {
      throw `Chrome 에 ‘${group.name}’ 이름의 탭 그룹이 열려 있지 않습니다. 같은 이름의 탭 그룹을 연 뒤 다시 해 주세요.`;
    }
    const tabs = [
      ...group.links.slice(0, 1).map((link) => ({ url: link.url, title: link.alias ?? "" })),
      { url: "https://status.example.com", title: L("상태 페이지") },
      { url: "https://docs.example.com/runbook", title: L("런북") },
    ];
    const urls = new Set(tabs.map((tab) => tab.url));
    return {
      browser: group.browser,
      tabGroupTitle: group.name,
      tabs,
      browserOnly: tabs.map((tab, index) => (group.links.some((link) => link.url === tab.url) ? -1 : index)).filter((index) => index >= 0),
      hereOnly: group.links.filter((link) => !urls.has(link.url)).map((link) => link.id),
      same: group.links.filter((link) => urls.has(link.url)).length,
    };
  },
  sync_group_apply: ({ id, choice }) => {
    const group = store.groups.find((item) => item.id === id);
    if (choice.replaceAll) {
      group.links = choice.tabs.map((tab) => group.links.find((link) => link.url === tab.url) ?? link(tab.url, choice.useTitleAsAlias ? tab.title : undefined));
    } else {
      group.links = group.links.filter((item) => !choice.remove.includes(item.id));
      for (const index of choice.add) {
        const tab = choice.tabs[index];
        group.links.push(link(tab.url, choice.useTitleAsAlias ? tab.title : undefined));
      }
    }
    changed();
  },
  set_search_scope: ({ scope }) => {
    searchScopeState = scope;
    emit("search-scope-changed", scope);
  },
  filter_group_links: ({ query, groupId }) => {
    const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
    const group = store.groups.find((item) => item.id === groupId);
    return (group?.links ?? []).filter((link) => terms.every((term) => `${link.alias ?? ""} ${link.url}`.toLowerCase().includes(term))).map((link) => link.id);
  },
  set_link_opens_with_group: ({ groupId, linkId, opens }) => {
    const link = store.groups.find((item) => item.id === groupId)?.links.find((item) => item.id === linkId);
    if (link) {
      if (opens) {
        delete link.opensWithGroup;
      } else {
        link.opensWithGroup = false;
      }
      changed();
    }
  },
  profiles: () => profileView(),
  switch_profile: ({ id }) => switchProfile(id),
  add_profile: ({ name }) => {
    const clean = profileName(name);
    if (sameProfileName(clean)) {
      throw "같은 이름의 프로필이 이미 있습니다.";
    }
    const id = newID();
    profileState.profiles.push({ id, name: clean, icon: PROFILE_ICON, color: "blue", isDefault: false });
    profilesChanged();
    return id;
  },
  update_profile: ({ id, name, icon, color }) => {
    const clean = profileName(name);
    if (sameProfileName(clean, id)) {
      throw "같은 이름의 프로필이 이미 있습니다.";
    }
    Object.assign(profileState.profiles.find((profile) => profile.id === id), { name: clean, icon, color });
    profilesChanged();
  },
  delete_profile: ({ id }) => {
    if (id === "default") {
      throw "기본 프로필은 지울 수 없습니다.";
    }
    if (profileState.current === id) {
      switchProfile("default");
    }
    profileState.profiles = profileState.profiles.filter((profile) => profile.id !== id);
    profileData.delete(id);
    profilesChanged();
  },
  set_language: () => null,
  set_theme: () => null,
  whats_new_content: () => ({
    title: "LinkGroupz 1.0.0 으로 업데이트했습니다",
    entries: [{ version: "1.0.0", changes: ["첫 공개 버전입니다. 자주 여는 웹 페이지를 그룹으로 묶어 Chrome · Edge · Whale 에 한 번에 엽니다", "새 버전이 나오면 앱 안에서 진행 막대를 보며 업데이트합니다"] }],
  }),
};

// 되돌리기: crates/app/src/history.rs 를 흉내 낸다 (바꾸는 명령 전후의 store 를 통째로 기억, 이름은 명령 이름).
const UNDOABLE = new Set([
  "add_group", "update_group", "set_group_browser", "set_group_always_private", "delete_groups", "move_groups", "move_items",
  "add_section", "rename_section", "delete_section", "move_section", "archive_groups", "add_link", "update_link", "add_links",
  "delete_links", "move_links_within", "move_links", "archive_links", "restore_groups", "restore_links", "restore_groups_to",
  "restore_archived_group_links", "delete_archived_group_links", "move_archived_links_into_group", "move_archived_groups",
  "move_archived_links", "move_links_in_archived_group", "archive_section", "restore_links_to_new_group", "delete_archived_groups",
  "delete_archived_links", "set_link_opens_with_group", "sync_group_apply", "import_tab_groups", "import_data",
]);
const undoStack = [];
const redoStack = [];
const historyStep = (from, to, pick) => {
  const entry = from.pop();
  if (!entry) {
    return null;
  }
  to.push(entry);
  Object.assign(store, structuredClone(pick(entry)));
  changed();
  return entry.label;
};
commands.undo = () => historyStep(undoStack, redoStack, (entry) => entry.before);
commands.redo = () => historyStep(redoStack, undoStack, (entry) => entry.after);

export const backend = {
  async invoke(command, args) {
    const handler = commands[command];
    if (!handler) {
      console.debug(`[가짜 앱] ${command}`, args ?? "");
      return null;
    }
    const before = UNDOABLE.has(command) ? JSON.stringify(store) : null;
    const result = structuredClone(await handler(args ?? {}));
    if (before !== null && before !== JSON.stringify(store)) {
      undoStack.push({ label: command, before: JSON.parse(before), after: structuredClone(store) });
      undoStack.splice(0, undoStack.length - 50);
      redoStack.length = 0;
    }
    return result;
  },
  listen(event, handler) {
    const handlers = listeners.get(event) ?? new Set();
    handlers.add(handler);
    listeners.set(event, handlers);
    return () => handlers.delete(handler);
  },
};
