// 여러 창이 함께 쓰는 화면 조각.

import { html, render, useEffect, useLayoutEffect, useRef, useState } from "../vendor/preact-htm.js";
import { boot, inApp, invoke, listen } from "./api.js";
import { lang, t } from "./i18n.js";

export { lang, t };
import { GROUP_SYMBOLS, ICON_PATHS } from "./icon-data.js";

export { boot, invoke, listen };
export * from "../vendor/preact-htm.js";

// MARK: - 값

const COLORS = new Map(boot.colors.map((color) => [color.id, color]));
const BROWSERS = new Map(boot.browsers.map((browser) => [browser.id, browser]));

export function colorInfo(id) {
  return COLORS.get(id) ?? COLORS.get("grey");
}

/** 그룹 색을 CSS 색으로 */
export function colorCss(id, alpha = 1) {
  const [red, green, blue] = colorInfo(id).rgb;
  return alpha >= 1 ? `rgb(${red}, ${green}, ${blue})` : `rgba(${red}, ${green}, ${blue}, ${alpha})`;
}

export function browserInfo(id) {
  return BROWSERS.get(id) ?? boot.browsers[0];
}

/** 페이지 이름: alias 가 있으면 alias, 없으면 호스트 + 경로 */
export function linkName(link) {
  if (link.alias) {
    return link.alias;
  }
  try {
    const url = new URL(link.url);
    if (!url.host) {
      return link.url;
    }
    let path = decodeURIComponent(url.pathname);
    if (path.length > 1 && path.endsWith("/")) {
      path = path.slice(0, -1);
    }
    return path === "/" || path === "" ? url.host : url.host + path;
  } catch {
    return link.url;
  }
}

/** Swift 의 Date 형식(2001-01-01 부터 지난 초)으로 적힌 시각 → Date */
export function fromTimestamp(seconds) {
  return new Date((seconds + 978307200) * 1000);
}

// 날짜 · 시각은 화면 언어의 형식으로 (11월 13일 · Nov 13)
const locale = lang === "en" ? "en-US" : "ko-KR";
const monthDay = new Intl.DateTimeFormat(locale, { month: "long", day: "numeric" });
const shortTime = new Intl.DateTimeFormat(locale, { hour: "numeric", minute: "2-digit" });

export function formatMonthDay(date) {
  return monthDay.format(date);
}

export function formatTime(date) {
  return shortTime.format(date);
}

export function classes(...names) {
  return names.filter(Boolean).join(" ");
}

/** Set 에서 값을 넣거나 뺀 새 Set */
export function toggled(set, value) {
  const next = new Set(set);
  if (next.has(value)) {
    next.delete(value);
  } else {
    next.add(value);
  }
  return next;
}

// MARK: - 그룹 · 보관함 (앱이 바꿀 때마다 알려 준다)

let storeSnapshot = await invoke("store_snapshot");
const storeListeners = new Set();

listen("store-changed", (snapshot) => {
  storeSnapshot = snapshot;
  for (const notify of storeListeners) {
    notify(snapshot);
  }
});

/** 그룹 · 보관함 전체. 바뀌면 다시 그린다. */
export function useStore() {
  const [snapshot, setSnapshot] = useState(storeSnapshot);
  useEffect(() => {
    storeListeners.add(setSnapshot);
    setSnapshot(storeSnapshot);
    return () => storeListeners.delete(setSnapshot);
  }, []);
  return snapshot;
}

/**
 * 검색어에 맞는 결과를 앱에 물어 온다 (검색 규칙은 앱 한곳에만 둔다).
 * 검색어가 비면 묻지 않고 fallback 을 그대로 쓴다.
 */
export function useFiltered(command, query, snapshot, fallback, extra) {
  const [result, setResult] = useState({ query: "", value: fallback });
  const scope = useSearchScope();
  const extraKey = JSON.stringify(extra ?? {});
  const trimmed = query.trim();
  useEffect(() => {
    if (!trimmed) {
      return undefined;
    }
    let current = true;
    invoke(command, { query, ...(extra ?? {}) }).then((value) => {
      if (current) {
        setResult({ query, value });
      }
    });
    return () => {
      current = false;
    };
  }, [command, query, snapshot, scope, extraKey]);
  if (!trimmed) {
    return fallback;
  }
  // 새 검색어의 답이 오기 전에는 지난 결과를 그대로 보여 준다 (목록이 깜빡이지 않게).
  return result.query ? result.value : fallback;
}

// MARK: - 섹션

/**
 * 그룹 목록의 맨 위 단계: 섹션(안의 그룹과 함께)과 섹션 밖 그룹을 화면 순서대로.
 * shown 을 주면 그 그룹만 남기고(검색 결과), 맞는 그룹이 없는 섹션은 뺀다.
 * 결과: [{ kind: "section", section, groups }, { kind: "group", group }]
 */
export function sidebarItems(store, shown) {
  const byID = new Map((shown ?? store.groups).map((group) => [group.id, group]));
  const items = [];
  for (const entry of store.sidebarOrder ?? []) {
    const split = entry.indexOf(":");
    const kind = entry.slice(0, split);
    const id = entry.slice(split + 1);
    if (kind === "group") {
      const group = byID.get(id);
      if (group) {
        items.push({ kind: "group", group });
      }
    } else if (kind === "section") {
      const section = store.sections.find((item) => item.id === id);
      if (!section) {
        continue;
      }
      // 섹션 안 순서는 그룹 목록 순서를 따른다.
      const groups = store.groups.filter((group) => group.sectionID === id && byID.has(group.id)).map((group) => byID.get(group.id));
      if (shown && groups.length === 0) {
        continue;
      }
      items.push({ kind: "section", section, groups });
    }
  }
  return items;
}

// MARK: - 검색어 강조

/** 앱의 검색 규칙(crates/core/src/search.rs)처럼 접는다: 대소문자 · 전각/반각 · 발음 구별 기호를 가리지 않는다. */
function foldChar(character) {
  return character.normalize("NFKD").replace(/\p{M}/gu, "").normalize("NFC").toLowerCase();
}

/** 검색어를 강조할 낱말로: text(이름 · 태그 · 페이지 이름 · URL 에서) · tags(태그에서만) */
export function searchTerms(query) {
  const text = [];
  const tags = [];
  for (const raw of query.split(/\s+/)) {
    const term = Array.from(raw).map(foldChar).join("");
    if (term.startsWith("#")) {
      const tag = term.replace(/^#+/, "");
      if (tag) {
        tags.push(tag);
      }
    } else if (term) {
      text.push(term);
    }
  }
  return { text, tags };
}

/** text 에서 terms 가 맞는 자리들 [[시작, 끝), …] (원래 글자 기준, 겹치면 합친다) */
function highlightRanges(text, terms) {
  if (!text || terms.length === 0) {
    return [];
  }
  let folded = "";
  const starts = []; // 접은 글자 하나하나가 원래 글자의 어디서 왔는지
  let position = 0;
  for (const character of Array.from(text)) {
    const fold = foldChar(character);
    for (let index = 0; index < fold.length; index += 1) {
      starts.push(position);
    }
    folded += fold;
    position += character.length;
  }
  starts.push(position);
  const ranges = [];
  for (const term of terms) {
    let from = 0;
    for (;;) {
      const found = folded.indexOf(term, from);
      if (found < 0) {
        break;
      }
      let end = found + term.length;
      // 한 글자를 접어 여러 글자가 된 경우 그 글자 끝까지 칠한다.
      while (end < starts.length - 1 && starts[end] === starts[end - 1]) {
        end += 1;
      }
      ranges.push([starts[found], starts[end]]);
      from = found + Math.max(1, term.length);
    }
  }
  ranges.sort((a, b) => a[0] - b[0]);
  const merged = [];
  for (const range of ranges) {
    const last = merged[merged.length - 1];
    if (last && range[0] <= last[1]) {
      last[1] = Math.max(last[1], range[1]);
    } else {
      merged.push([...range]);
    }
  }
  return merged;
}

/** 검색어와 맞는 글자를 노랗게 칠한다. terms 가 비면 글자 그대로. */
export function Highlight({ text, terms }) {
  const ranges = terms?.length ? highlightRanges(text ?? "", terms) : [];
  if (ranges.length === 0) {
    return text ?? "";
  }
  const parts = [];
  let last = 0;
  for (const [start, end] of ranges) {
    if (start > last) {
      parts.push(text.slice(last, start));
    }
    parts.push(html`<mark>${text.slice(start, end)}</mark>`);
    last = end;
  }
  parts.push(text.slice(last));
  return parts;
}

// MARK: - 아이콘

export function Icon({ name, size = 16, class: className, title }) {
  const paths = ICON_PATHS[name] ?? ICON_PATHS["globe"];
  return html`<svg class=${classes("icon", className)} width=${size} height=${size} viewBox="0 0 256 256" aria-hidden=${title ? undefined : "true"}>
    ${title && html`<title>${title}</title>`}
    ${paths.map((d) => html`<path d=${d} />`)}
  </svg>`;
}

/** 시크릿 모드 아이콘 (모자 쓰고 안경 낀 사람) */
export function IncognitoIcon({ size = 16 }) {
  return html`<${Icon} name="detective-fill" size=${size} title=${t("시크릿 모드")} />`;
}

/** 한 번 받은 것은 다시 받지 않는다 (이름 → 값을 기다리는 Promise). hint 는 처음 받을 때만 쓰는 덧붙임 값. */
function cached(load) {
  const promises = new Map();
  const values = new Map();
  return {
    peek: (key) => values.get(key),
    get(key, hint) {
      if (!promises.has(key)) {
        promises.set(
          key,
          load(key, hint)
            .catch(() => null)
            .then((value) => {
              values.set(key, value);
              return value;
            }),
        );
      }
      return promises.get(key);
    },
  };
}

/** 캐시에서 값을 꺼내 쓴다. 아직 없으면 undefined 였다가 오면 다시 그린다. */
function useCached(cache, key, hint) {
  const [value, setValue] = useState(() => (key == null ? null : cache.peek(key)));
  useEffect(() => {
    if (key == null) {
      setValue(null);
      return undefined;
    }
    const known = cache.peek(key);
    setValue(known);
    if (known !== undefined) {
      return undefined;
    }
    let current = true;
    cache.get(key, hint).then((loaded) => {
      if (current) {
        setValue(loaded);
      }
    });
    return () => {
      current = false;
    };
  }, [key]);
  return value;
}

const groupImages = cached((name) => invoke("group_icon_image", { name }));
const browserIcons = cached((key) => {
  const [browser, size] = key.split("@");
  return invoke("browser_icon", { browser, size: Number(size) });
});
// 사이트마다 처음 본 페이지 주소로 아이콘을 찾는다 (루트에 아이콘이 없으면 그 페이지의 HTML 에서 찾는다).
const favicons = cached((site, pageUrl) => invoke("favicon", { url: pageUrl ?? site }));

/** 그룹 아이콘: 그룹 색의 둥근 사각형에 흰 기호, 또는 사용자가 넣은 이미지 */
export function GroupIcon({ icon, color, size = 30 }) {
  const imageName = icon?.image ?? null;
  const image = useCached(groupImages, imageName);
  const style = `width:${size}px;height:${size}px;border-radius:${size * 0.3}px;background:${colorCss(color)}`;
  if (imageName && image) {
    return html`<span class="group-icon" style=${style}><img src=${image} alt="" draggable=${false} /></span>`;
  }
  // 이미지 파일이 없어졌거나 모르는 기호면 기본 폴더로 그린다.
  const symbol = GROUP_SYMBOLS[icon?.symbol] ?? GROUP_SYMBOLS[boot.defaultSymbol];
  return html`<span class="group-icon" style=${style}><${Icon} name=${symbol} size=${Math.round(size * 0.58)} /></span>`;
}

/** 그룹 편집에서 고르는 기본 모양 하나 */
export function SymbolIcon({ symbol, size = 16 }) {
  return html`<${Icon} name=${GROUP_SYMBOLS[symbol] ?? GROUP_SYMBOLS[boot.defaultSymbol]} size=${size} />`;
}

/** 브라우저 아이콘 (설치돼 있지 않으면 지구본) */
export function BrowserIcon({ browser, size = 14 }) {
  const info = browserInfo(browser);
  const icon = useCached(browserIcons, `${browser}@${size}`);
  return html`<span class="browser-icon" style=${`width:${size}px;height:${size}px`} title=${info.name}>
    ${icon ? html`<img src=${icon} alt=${info.name} draggable=${false} />` : html`<${Icon} name="globe" size=${Math.round(size * 0.9)} />`}
  </span>`;
}

/** 그룹 목록에서 그룹을 열 브라우저를 보여준다. */
export function BrowserBadge({ browser, size = 14 }) {
  return html`<span title=${t("{0} 에서 열림", browserInfo(browser).name)}><${BrowserIcon} browser=${browser} size=${size} /></span>`;
}

function siteOf(url) {
  try {
    const parsed = new URL(url);
    return parsed.protocol === "http:" || parsed.protocol === "https:" ? parsed.origin : null;
  } catch {
    return null;
  }
}

/** 페이지 이름 앞의 사이트 아이콘. 아이콘은 사이트(호스트)마다 한 번만 받는다. */
export function Favicon({ url, size = 16 }) {
  const icon = useCached(favicons, siteOf(url), url);
  return html`<span class="favicon" style=${`width:${size}px;height:${size}px`}>
    ${icon ? html`<img src=${icon} alt="" draggable=${false} />` : html`<${Icon} name="globe" size=${size} />`}
  </span>`;
}

export function ColorDot({ color, size = 10 }) {
  return html`<span class="dot" style=${`width:${size}px;height:${size}px;background:${colorCss(color)}`}></span>`;
}

export function TagChips({ tags, terms }) {
  if (!tags?.length) {
    return null;
  }
  const tagTerms = terms ? [...terms.text, ...terms.tags] : [];
  return html`<span class="chips">${tags.map((tag) => html`<span class="chip" key=${tag}>#<${Highlight} text=${tag} terms=${tagTerms} /></span>`)}</span>`;
}

// MARK: - 입력

/**
 * 체크박스. state 는 "on" · "off" · "mixed"(일부만 고름, − 로 보인다).
 * 글자(children)까지 눌러도 된다.
 */
export function Check({ state, onToggle, disabled, title, children }) {
  const icon = state === "on" ? "check-square-fill" : state === "mixed" ? "minus-square-fill" : "square";
  const click = (event) => {
    event.stopPropagation();
    onToggle?.(event);
  };
  return html`<button type="button" class=${classes("check", state)} disabled=${disabled} title=${title} onClick=${click}>
    <${Icon} name=${icon} size=${16} class="box" />${children}
  </button>`;
}

/** 여러 개를 한 번에 고르는 체크박스. 누를 때마다 전체 선택 ↔ 전체 해제. 일부만 골랐으면 누르면 전체 선택. */
export function SelectAll({ selected, selectable, onSelectAll, onDeselectAll, children }) {
  const all = selectable > 0 && selected >= selectable;
  return html`<${Check}
    state=${all ? "on" : selected > 0 ? "mixed" : "off"}
    disabled=${selectable === 0}
    title=${all ? t("전체 해제") : t("전체 선택")}
    onToggle=${all ? onDeselectAll : onSelectAll}
  >${children}<//>`;
}

/** 접기 · 펼치기 화살표 */
export function Chevron({ expanded, onToggle, disabled }) {
  return html`<button type="button" class="chevron" disabled=${disabled} title=${expanded ? t("접기") : t("펼치기")} onClick=${(event) => {
    event.stopPropagation();
    onToggle();
  }}>
    <${Icon} name=${expanded ? "caret-down-bold" : "caret-right-bold"} size=${12} />
  </button>`;
}

/**
 * 검색창.
 *
 * 입력하는 동안에는 바깥 값으로 입력창을 덮어쓰지 않는다. 한글처럼 조합해서 입력하는 글자가
 * 조합 도중에 다시 쓰이면 글자가 끊기기 때문이다. 바깥에서 값을 바꾼 경우(지우기 등)에만 맞춘다.
 */
export function SearchField({ value, onChange, placeholder, autofocus, focusSignal, onKeyDown, inputClass }) {
  const input = useRef(null);
  useLayoutEffect(() => {
    if (input.current && input.current.value !== value && document.activeElement !== input.current) {
      input.current.value = value;
    }
  }, [value]);
  useEffect(() => {
    if (autofocus) {
      input.current?.focus();
    }
  }, [autofocus, focusSignal]);
  const clear = () => {
    input.current.value = "";
    onChange("");
    input.current.focus();
  };
  return html`<div class="search">
    <${Icon} name="magnifying-glass" size=${14} class="lens" />
    <input
      ref=${input}
      class="field"
      type="text"
      spellcheck=${false}
      autocomplete="off"
      placeholder=${placeholder}
      defaultValue=${value}
      onInput=${(event) => onChange(event.target.value)}
      data-search=${inputClass}
      onKeyDown=${(event) => {
        if (event.key === "Escape" && event.target.value) {
          event.stopPropagation();
          clear();
          return;
        }
        onKeyDown?.(event);
      }}
    />
    ${value && html`<button type="button" class="btn plain clear" title=${t("지우기")} onClick=${clear}><${Icon} name="x-circle-fill" size=${14} /></button>`}
  </div>`;
}

/** Chrome · Edge · Whale 고르기. 설치되지 않은 브라우저는 고를 수 없다. */
export function BrowserPicker({ value, onChange, states }) {
  return html`<div class="row" style="gap:6px">
    ${boot.browsers.map((browser) => {
      const state = states?.find((item) => item.id === browser.id);
      const installed = state ? state.installed : browser.installed;
      return html`<button
        type="button"
        key=${browser.id}
        class="btn"
        style=${value === browser.id ? "background:var(--accent-soft);border-color:var(--accent)" : ""}
        disabled=${!installed}
        title=${installed ? browser.fullName : t("설치돼 있지 않음: {0}", browser.fullName)}
        onClick=${() => onChange(browser.id)}
      >
        <${BrowserIcon} browser=${browser.id} size=${16} />${browser.name}
        ${state && installed && !state.running && html`<span class="caption">${t("꺼짐")}</span>`}
      </button>`;
    })}
  </div>`;
}

// MARK: - 메뉴

let menuHost = null;

function closeMenu() {
  if (menuHost) {
    render(null, menuHost);
    menuHost.remove();
    menuHost = null;
    // 메뉴 자리를 위해 늘렸던 창(메뉴바 창)을 되돌린다.
    window.dispatchEvent(new CustomEvent("menu-room", { detail: 0 }));
  }
}

/**
 * 메뉴를 띄운다. at 은 마우스 이벤트(그 자리) 또는 요소(그 아래).
 *
 * items: { label, action, disabled, checked, destructive, submenu } 또는 "-"(구분선). 빈 값은 건너뛴다.
 * 앱 안에서는 운영체제 기본 메뉴(macOS · Windows)로 띄우고, 브라우저로 화면만 열어 볼 때(가짜 앱)는 화면 안에 그린다.
 */
export function showMenu(at, items) {
  if (inApp) {
    showNativeMenu(at, items);
    return;
  }
  closeMenu();
  let x;
  let y;
  if (at instanceof Element) {
    const box = at.getBoundingClientRect();
    x = box.left;
    y = box.bottom + 4;
  } else {
    at.preventDefault();
    x = at.clientX;
    y = at.clientY;
  }
  menuHost = document.createElement("div");
  document.body.append(menuHost);
  render(html`<${MenuLayer} x=${x} y=${y} items=${items} />`, menuHost);
}

/** 맨 앞 · 맨 뒤 · 연달아 나오는 구분선과 빈 값을 뺀다. */
function tidyMenuItems(items) {
  const shown = [];
  for (const item of items.filter(Boolean)) {
    if (item === "-" && (shown.length === 0 || shown[shown.length - 1] === "-")) {
      continue;
    }
    shown.push(item);
  }
  while (shown[shown.length - 1] === "-") {
    shown.pop();
  }
  return shown;
}

/** 운영체제 기본 메뉴. 고르면 그 항목의 action 을 부른다. 체크 표시가 있는 항목(checked 값이 있는 것)은 체크 항목으로 만든다. */
async function showNativeMenu(at, items) {
  const { Menu, Submenu, MenuItem, CheckMenuItem, PredefinedMenuItem } = window.__TAURI__.menu;
  const { LogicalPosition } = window.__TAURI__.dpi;
  // 자리는 이벤트가 살아 있을 때 읽는다.
  let position;
  if (at instanceof Element) {
    const box = at.getBoundingClientRect();
    position = new LogicalPosition(box.left, box.bottom + 4);
  } else {
    at.preventDefault();
    position = new LogicalPosition(at.clientX, at.clientY);
  }
  const build = (list) =>
    Promise.all(
      tidyMenuItems(list).map(async (item) => {
        if (item === "-") {
          return PredefinedMenuItem.new({ item: "Separator" });
        }
        const enabled = !item.disabled;
        if (item.submenu) {
          return Submenu.new({ text: item.label, enabled, items: await build(item.submenu) });
        }
        const action = () => item.action?.();
        return "checked" in item
          ? CheckMenuItem.new({ text: item.label, enabled, checked: Boolean(item.checked), action })
          : MenuItem.new({ text: item.label, enabled, action });
      }),
    );
  const menu = await Menu.new({ items: await build(items) });
  await menu.popup(position);
}

/** 키보드로 메뉴 고르기: 열린 메뉴(하위 메뉴가 열려 있으면 가장 안쪽)가 키를 받는다. */
const menuKeyHandlers = [];
const MENU_KEYS = new Set(["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight", "Enter", " ", "Tab"]);

function MenuLayer({ x, y, items }) {
  useEffect(() => {
    const onKey = (event) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        closeMenu();
        return;
      }
      if (MENU_KEYS.has(event.key) && !isComposingKey(event)) {
        // 메뉴가 떠 있는 동안 화살표 · Return 은 뒤쪽 목록으로 가지 않는다.
        event.preventDefault();
        event.stopPropagation();
        menuKeyHandlers[menuKeyHandlers.length - 1]?.(event.key);
      }
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", closeMenu);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("blur", closeMenu);
    };
  }, []);
  const dismiss = (event) => {
    if (event.target === event.currentTarget) {
      event.preventDefault();
      closeMenu();
    }
  };
  return html`<div class="menu-layer" onMouseDown=${dismiss} onContextMenu=${dismiss}>
    <${MenuList} x=${x} y=${y} items=${items} />
  </div>`;
}

function MenuList({ x, y, items, alignRightOf, startActive, onCloseSelf }) {
  const list = useRef(null);
  const [position, setPosition] = useState({ left: x, top: y, visibility: "hidden" });
  const [open, setOpen] = useState(null);
  // 키보드로 고른 항목 (마우스를 올려도 바뀐다)
  const [active, setActive] = useState(null);

  // 화면 밖으로 나가지 않게 자리를 고친다. 창이 작으면(메뉴바 창) 창을 늘려 달라고 알리고, 창이 커지면 다시 맞춘다.
  const place = () => {
    const element = list.current;
    if (!element) {
      return;
    }
    const box = element.getBoundingClientRect();
    const height = element.scrollHeight;
    window.dispatchEvent(new CustomEvent("menu-room", { detail: y + height + 12 }));
    let left = x;
    let top = y;
    if (left + box.width > window.innerWidth - 8) {
      left = alignRightOf != null ? alignRightOf - box.width : window.innerWidth - box.width - 8;
    }
    if (top + Math.min(height, window.innerHeight - 16) > window.innerHeight - 8) {
      top = window.innerHeight - Math.min(height, window.innerHeight - 16) - 8;
    }
    setPosition({ left: Math.max(8, left), top: Math.max(8, top), visibility: "visible" });
  };
  useLayoutEffect(place, [x, y]);
  useEffect(() => {
    window.addEventListener("resize", place);
    return () => window.removeEventListener("resize", place);
  }, [x, y]);

  const shown = items.filter(Boolean);
  const usable = (index) => shown[index] && shown[index] !== "-" && !shown[index].disabled;

  // 키보드로 연 하위 메뉴는 첫 항목을 골라 둔다.
  useEffect(() => {
    if (startActive) {
      setActive(shown.findIndex((_, index) => usable(index)));
    }
  }, []);

  const openSubmenu = (index) => {
    const element = list.current?.querySelector(`:scope > [data-index="${index}"]`);
    if (!element) {
      return;
    }
    const box = element.getBoundingClientRect();
    setOpen({ index, x: box.right - 2, y: box.top - 5, left: box.left + 2, keyboard: true });
  };

  // 가장 안쪽 메뉴가 키를 받는다 (하위 메뉴가 열리면 그쪽이 위에 쌓인다).
  const handleKey = useRef(null);
  handleKey.current = (key) => {
    if (key === "ArrowDown" || key === "ArrowUp") {
      const step = key === "ArrowDown" ? 1 : -1;
      let index = active ?? (step > 0 ? -1 : shown.length);
      for (let tries = 0; tries < shown.length; tries += 1) {
        index = (index + step + shown.length) % shown.length;
        if (usable(index)) {
          setActive(index);
          setOpen(null);
          break;
        }
      }
    } else if ((key === "Enter" || key === " " || key === "ArrowRight") && active != null && usable(active)) {
      const item = shown[active];
      if (item.submenu) {
        openSubmenu(active);
      } else if (key !== "ArrowRight") {
        closeMenu();
        item.action?.();
      }
    } else if (key === "ArrowLeft") {
      onCloseSelf?.();
    }
  };
  useEffect(() => {
    const handler = (key) => handleKey.current(key);
    menuKeyHandlers.push(handler);
    return () => menuKeyHandlers.splice(menuKeyHandlers.indexOf(handler), 1);
  }, []);

  return html`<div ref=${list} class="menu" style=${`left:${position.left}px;top:${position.top}px;visibility:${position.visibility}`}>
    ${shown.map((item, index) => {
      if (item === "-") {
        // 맨 앞 · 맨 뒤 · 연달아 나오는 구분선은 그리지 않는다.
        const previous = shown[index - 1];
        return index === 0 || index === shown.length - 1 || previous === "-" ? null : html`<div class="separator" key=${index}></div>`;
      }
      const hasSubmenu = Boolean(item.submenu);
      const enter = (event) => {
        setActive(item.disabled ? null : index);
        if (!hasSubmenu || item.disabled) {
          setOpen(null);
          return;
        }
        const box = event.currentTarget.getBoundingClientRect();
        setOpen({ index, x: box.right - 2, y: box.top - 5, left: box.left + 2 });
      };
      const click = () => {
        if (hasSubmenu) {
          return;
        }
        closeMenu();
        item.action?.();
      };
      return html`<button
        type="button"
        key=${index}
        data-index=${index}
        class=${classes("item", item.destructive && "destructive", (open?.index === index || active === index) && "open")}
        disabled=${item.disabled}
        onMouseEnter=${enter}
        onClick=${click}
      >
        <span class="mark">${item.checked && html`<${Icon} name="check-bold" size=${12} />`}</span>
        <span class="label">${item.label}</span>
        ${hasSubmenu && html`<${Icon} name="caret-right-bold" size=${10} />`}
      </button>`;
    })}
    ${open &&
    html`<${MenuList}
      key=${open.index}
      x=${open.x}
      y=${open.y}
      alignRightOf=${open.left}
      items=${shown[open.index].submenu}
      startActive=${open.keyboard}
      onCloseSelf=${() => setOpen(null)}
    />`}
  </div>`;
}

// MARK: - 프로필

/** 프로필 목록 · 지금 프로필 ({ profiles: [{ id, name, isDefault, groups, links }], current }) */
let profileState = boot.profiles;
const profileListeners = new Set();
listen("profiles-changed", (state) => {
  profileState = state;
  for (const notify of profileListeners) {
    notify(state);
  }
});

export function useProfiles() {
  const [state, setState] = useState(profileState);
  useEffect(() => {
    profileListeners.add(setState);
    setState(profileState);
    return () => profileListeners.delete(setState);
  }, []);
  return state;
}

export function currentProfile(state) {
  return state.profiles.find((profile) => profile.id === state.current) ?? state.profiles[0];
}

/** 프로필 바꾸기 메뉴. 추가 · 이름 바꾸기 · 삭제는 설정 창에서만 한다. */
export function profileMenuItems(state) {
  return [
    ...state.profiles.map((profile) => ({
      label: profile.name,
      checked: profile.id === state.current,
      action: () => profile.id !== state.current && invoke("switch_profile", { id: profile.id }),
    })),
    "-",
    { label: t("프로필 추가 · 관리…"), action: () => invoke("show_settings") },
  ];
}

/** 지금 프로필 이름이 적힌 버튼. 누르면 다른 프로필로 바꾼다 (모든 창에 같은 버튼이 있다). */
export function ProfileSwitcher({ class: className }) {
  const state = useProfiles();
  const profile = currentProfile(state);
  return html`<${MenuButton}
    class=${classes("btn small profile-switcher", className)}
    title=${t("프로필 바꾸기 — 프로필마다 그룹 목록이 따로 있습니다")}
    items=${() => profileMenuItems(state)}
  >
    <${GroupIcon} icon=${profile?.icon} color=${profile?.color} size=${16} />
    <span class="truncate">${profile?.name}</span>
    <${Icon} name="caret-down-bold" size=${9} />
  <//>`;
}

// MARK: - 검색할 곳 (이름 · 태그 · URL)

let searchScope = boot.searchScope ?? { name: true, tags: true, url: true };
const scopeListeners = new Set();
listen("search-scope-changed", (scope) => {
  searchScope = scope;
  for (const notify of scopeListeners) {
    notify(scope);
  }
});

export function useSearchScope() {
  const [scope, setScope] = useState(searchScope);
  useEffect(() => {
    scopeListeners.add(setScope);
    setScope(searchScope);
    return () => scopeListeners.delete(setScope);
  }, []);
  return scope;
}

const SCOPE_PARTS = [
  ["name", t("이름 (그룹 · 페이지)")],
  ["tags", t("태그")],
  ["url", "URL"],
];

/** 검색창 옆 거르기 버튼: 이름 · 태그 · URL 중 어디서 찾을지 (관리 창 · 메뉴바 · 보관함이 함께 쓴다) */
export function SearchScopeButton() {
  const scope = useSearchScope();
  const all = SCOPE_PARTS.every(([key]) => scope[key]);
  const on = SCOPE_PARTS.filter(([key]) => scope[key]).map(([, label]) => label.split(" ")[0]);
  const items = () => [
    ...SCOPE_PARTS.map(([key, label]) => ({
      label,
      checked: scope[key],
      // 하나는 늘 켜 둔다.
      disabled: scope[key] && on.length === 1,
      action: () => invoke("set_search_scope", { scope: { ...scope, [key]: !scope[key] } }),
    })),
    "-",
    { label: t("모두 켜기"), disabled: all, action: () => invoke("set_search_scope", { scope: { name: true, tags: true, url: true } }) },
  ];
  return html`<${MenuButton}
    class=${classes("btn plain icon-only scope-button", !all && "on")}
    title=${t("검색할 곳: {0} — 눌러서 바꾸기", on.join(" · "))}
    items=${items}
  ><${Icon} name="funnel" size=${15} /><//>`;
}

/** 작은 켜기 · 끄기 스위치 (페이지 줄의 '그룹 열 때 열기') */
export function MiniSwitch({ on, onChange, title }) {
  return html`<button
    type="button"
    class=${classes("mini-switch", on && "on")}
    role="switch"
    aria-checked=${on}
    title=${title}
    onClick=${(event) => {
      event.stopPropagation();
      onChange(!on);
    }}
    onDblClick=${(event) => event.stopPropagation()}
  ></button>`;
}

/** 누르면 아래에 메뉴가 뜨는 버튼 */
export function MenuButton({ items, class: className = "btn", title, disabled, children }) {
  const open = (event) => showMenu(event.currentTarget, typeof items === "function" ? items() : items);
  return html`<button type="button" class=${className} title=${title} disabled=${disabled} onClick=${open}>${children}</button>`;
}

/** 브라우저 고르기 메뉴 항목 (현재 값에 체크) */
export function browserMenuItems(selected, onSelect) {
  return boot.browsers.map((browser) => ({
    label: browser.name,
    checked: browser.id === selected,
    disabled: !browser.installed,
    action: () => onSelect(browser.id),
  }));
}

// MARK: - 시트 · 확인

/**
 * 글자를 조합하는 중(한글 · 일본어 · 중국어 입력)에 누른 키인지.
 *
 * WebKit(macOS WKWebView)은 조합을 끝내는 Return 의 keydown 을 compositionend 뒤에 isComposing=false 로 보내므로
 * isComposing 만 보면 한글 이름을 입력하고 Return 으로 확정하는 순간 시트가 저장돼 버린다.
 * 이때 keyCode 는 229 이고, compositionend 직후에 온다.
 */
let lastCompositionEnd = -Infinity;
window.addEventListener("compositionend", () => {
  lastCompositionEnd = performance.now();
}, true);

export function isComposingKey(event) {
  return event.isComposing || event.keyCode === 229 || performance.now() - lastCompositionEnd < 60;
}

/** 되돌리기 키 이름 (안내 글에 쓴다) */
export const UNDO_KEY = boot.platform === "macos" ? "⌘Z" : "Ctrl+Z";

/** 글자를 입력하는 중인 칸인지 (그 칸에서는 ⌘Z 가 입력한 글자를 되돌린다). 비어 있는 검색창은 빼다 — 메뉴바 창은 늘 검색창에 커서가 있다. */
function typingIn(target) {
  if (!(target instanceof Element)) {
    return false;
  }
  if (target.closest("textarea, select, [contenteditable='true'], [contenteditable='']")) {
    return true;
  }
  if (!(target instanceof HTMLInputElement) || ["checkbox", "radio", "button", "submit", "range", "color"].includes(target.type)) {
    return false;
  }
  return !(target.closest(".search") && target.value === "");
}

/**
 * 되돌리기 · 다시 실행 키 (crates/app/src/history.rs): macOS ⌘Z · ⌘⇧Z, Windows Ctrl+Z · Ctrl+Y · Ctrl+Shift+Z.
 * 글자를 입력하는 칸에서는 그 칸의 글자를 되돌리고, 편집 창 · 메뉴가 열려 있으면 받지 않는다.
 * 무엇을 되돌렸는지 알림 글을 onNotice 로 넘긴다.
 * macOS 는 편집 메뉴가 ⌘Z 를 먼저 가져가므로 메뉴가 보내는 알림(edit-undo-redo)으로 받고, Windows 는 키를 바로 받는다.
 */
export function useUndoKeys(onNotice) {
  const latest = useRef(onNotice);
  latest.current = onNotice;
  useEffect(() => {
    const mac = boot.platform === "macos";
    const runApp = (redo) =>
      invoke(redo ? "redo" : "undo").then((label) =>
        latest.current(label ? `${redo ? t("다시 실행했습니다") : t("되돌렸습니다")}: ${label}` : redo ? t("다시 실행할 일이 없습니다") : t("되돌릴 일이 없습니다")),
      );
    const onKey = (event) => {
      if (event.defaultPrevented || !(mac ? event.metaKey : event.ctrlKey) || event.altKey || isComposingKey(event)) {
        return;
      }
      // 한글 입력 상태에서도 같은 키로 보도록 자리(code)로 본다.
      const key = event.code === "KeyZ" ? "z" : event.code === "KeyY" ? "y" : event.key.toLowerCase();
      const redo = (key === "z" && event.shiftKey) || (!mac && key === "y" && !event.shiftKey);
      const undo = key === "z" && !event.shiftKey;
      if ((!undo && !redo) || typingIn(event.target) || document.querySelector(".sheet-layer, .menu-layer")) {
        return;
      }
      event.preventDefault();
      runApp(redo);
    };
    // macOS 편집 메뉴의 Undo · Redo: 포커스가 있는 창만 받는다.
    const onMenu = (redo) => {
      if (!document.hasFocus()) {
        return;
      }
      if (typingIn(document.activeElement)) {
        invoke("native_edit", { redo });
      } else if (!document.querySelector(".sheet-layer, .menu-layer")) {
        runApp(redo);
      }
    };
    window.addEventListener("keydown", onKey);
    const unlisten = listen("edit-undo-redo", onMenu);
    return () => {
      window.removeEventListener("keydown", onKey);
      unlisten();
    };
  }, []);
}

/**
 * 창 안에 뜨는 대화 상자.
 * Esc 는 취소(onClose), Return 은 기본 버튼(onSubmit) — 글자를 조합하는 중의 Return 은 넘기지 않는다.
 */
export function Sheet({ width, onClose, onSubmit, canSubmit = true, class: className, children }) {
  const layer = useRef(null);
  useEffect(() => {
    const onKey = (event) => {
      if (event.defaultPrevented || isComposingKey(event) || document.querySelector(".menu-layer")) {
        return;
      }
      // 시트가 겹쳐 있으면 맨 위 시트만 반응한다.
      const layers = document.querySelectorAll(".sheet-layer");
      if (layers[layers.length - 1] !== layer.current) {
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        onClose?.();
      } else if (event.key === "Enter" && onSubmit && canSubmit && event.target.tagName !== "BUTTON" && event.target.tagName !== "TEXTAREA") {
        event.preventDefault();
        onSubmit();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
  useEffect(() => {
    // 처음 입력 칸에 커서를 둔다.
    const first = layer.current?.querySelector("input[data-autofocus], input.field:not([type=checkbox])");
    first?.focus();
  }, []);
  return html`<div class="sheet-layer" ref=${layer}>
    <div class=${classes("sheet", className)} style=${width ? `width:${width}px` : ""} role="dialog" aria-modal="true">${children}</div>
  </div>`;
}

/** 되돌릴 수 없는 일을 하기 전에 묻는다. */
export function Confirm({ title, message, confirmLabel, onConfirm, onCancel }) {
  return html`<${Sheet} class="confirm" onClose=${onCancel}>
    <div class="title">${title}</div>
    ${message && html`<div class="secondary">${message}</div>`}
    <div class="actions">
      <button type="button" class="btn destructive" onClick=${onConfirm}>${confirmLabel}</button>
      <button type="button" class="btn" onClick=${onCancel}>${t("취소")}</button>
    </div>
  <//>`;
}

/** 빈 상태 안내 */
export function Empty({ icon, title, description, children }) {
  return html`<div class="empty">
    ${icon && html`<${Icon} name=${icon} size=${44} class="big" />`}
    <div class="title">${title}</div>
    ${description && html`<div>${description}</div>`}
    ${children && html`<div class="row" style="margin-top:6px">${children}</div>`}
  </div>`;
}

// MARK: - 그룹 카드

/** 그룹 카드 한 줄: 아이콘 · 이름 · 태그 · 페이지 수 · 브라우저 (브라우저는 항상 맨 오른쪽). terms: 강조할 검색어 */
export function GroupCardContent({ group, linkCount, terms }) {
  const tagTerms = terms ? [...terms.text, ...terms.tags] : [];
  return html`
    <${GroupIcon} icon=${group.icon} color=${group.color} size=${26} />
    <span class="names">
      <span class="name truncate"><${Highlight} text=${group.name} terms=${terms?.text} /></span>
      ${group.tags.length > 0 &&
      html`<span class="tags truncate">${group.tags.map((tag, index) => html`${index > 0 ? " " : ""}#<${Highlight} text=${tag} terms=${tagTerms} />`)}</span>`}
    </span>
    ${group.alwaysPrivate && html`<span class="secondary" title=${t("항상 시크릿 모드로 엽니다")}><${IncognitoIcon} size=${15} /></span>`}
    <span class="links-count">${linkCount ?? group.links.length}</span>
    <${BrowserBadge} browser=${group.browser} size=${15} />
  `;
}

/** 화면이 다 그려진 뒤(글꼴 · 그림 자리까지 잡힌 뒤) 한 번 부른다. */
export function afterPaint(work) {
  requestAnimationFrame(() => requestAnimationFrame(work));
}

/**
 * 창 높이를 내용(#app) 높이에 맞추고, 내용 높이가 달라질 때마다 다시 맞춘다. 그만둘 때 부를 함수를 돌려준다.
 *
 * - 내용 높이만 알려 주면 안 된다. 창에는 제목 표시줄이 있고 그 높이는 운영체제 · 버전마다 달라,
 *   지금 화면에 보이는 높이(window.innerHeight)도 함께 알려 모자란 만큼만 창을 키우거나 줄이게 한다.
 * - 처음 한 번만 재면 안 된다. 창이 막 뜰 때는 화면 폭이 아직 자리 잡지 않아 긴 글이 한 줄에 놓인 채로 재어진다.
 */
export function fitWindowToContent() {
  const app = document.getElementById("app");
  let scheduled = false;
  const fit = () => {
    scheduled = false;
    const content = Math.ceil(app.getBoundingClientRect().height);
    const viewport = window.innerHeight;
    if (content > 0 && content !== viewport) {
      invoke("fit_window_height", { content, viewport });
    }
  };
  const schedule = () => {
    if (!scheduled) {
      scheduled = true;
      requestAnimationFrame(fit);
    }
  };
  // 내용이 바뀌었을 때, 그리고 창 크기가 바뀐 뒤(맞춘 결과가 아직 모자라면 한 번 더)
  const observer = new ResizeObserver(schedule);
  observer.observe(app);
  window.addEventListener("resize", schedule);
  afterPaint(fit);
  return () => {
    observer.disconnect();
    window.removeEventListener("resize", schedule);
  };
}
