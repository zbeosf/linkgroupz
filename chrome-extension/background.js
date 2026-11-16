// LinkGroupz 앱과 마커 URL(https://linkgroupz.invalid/...)로 주고받는다.
//
//   /open#<base64url JSON {name, color, urls}>
//       페이지들을 이름 · 색이 붙은 탭 그룹으로 연다.
//       앱 쪽: crates/core/src/launcher.rs (TabGroupMarker)
//   /snapshot#<nonce>
//       모든 창의 탭과 탭 그룹을 모아 같은 탭을 /result#<base64url JSON> 으로 바꾼다.
//       앱(macOS)이 AppleScript 로 그 URL 을 읽고 탭을 닫는다. 아래 자동 저장을 못 쓸 때의 대체 경로다.
//       앱 쪽: crates/core/src/snapshot.rs (snapshot_via_marker_tab)
//   /push#<nonce>
//       모든 창의 탭과 탭 그룹을 nonce 와 함께 Native Messaging 호스트로 보내고 이 탭을 닫는다.
//       AppleScript 가 없는 Windows 에서 쓰는 대체 경로다. 앱은 호스트가 쓴 파일에서 nonce 를 확인한다.
//       앱 쪽: crates/core/src/snapshot.rs (snapshot_via_push)
//
// 자동 저장: 탭 · 탭 그룹 · 창이 바뀌면 같은 스냅샷을 Native Messaging 호스트(dev.linkgroupz.snapshot)로 보낸다.
//       호스트가 앱 데이터 폴더의 {browser}-snapshot.json 에 쓰고, 앱은 그 파일만 읽어 브라우저에 탭을 열지 않는다.
//       Chrome · Edge · Whale 에 같은 확장을 넣어도 되도록 스냅샷에 browser("chrome" | "edge" | "whale")를 넣는다.
//       앱 쪽: crates/core/src/native.rs, crates/host/src/main.rs
//
// 이름(마커 호스트 · Native Messaging 호스트 · manifest.json 의 name)은 crates/core/src/brand.rs 와 같아야 한다.

const MARKER_HOST = "linkgroupz.invalid";
const COLORS = new Set(["grey", "blue", "red", "yellow", "green", "pink", "purple", "cyan", "orange"]);
const handledTabs = new Set();
// Edge 의 user agent 에는 "Edg/", Whale 에는 "Whale/" 이 들어 있다.
const BROWSER = navigator.userAgent.includes("Edg/") ? "edge" : navigator.userAgent.includes("Whale/") ? "whale" : "chrome";

function decodeBase64URL(text) {
  let encoded = text.replace(/-/g, "+").replace(/_/g, "/");
  while (encoded.length % 4 !== 0) {
    encoded += "=";
  }
  const bytes = Uint8Array.from(atob(encoded), (c) => c.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

function encodeBase64URL(text) {
  const bytes = new TextEncoder().encode(text);
  let binary = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** 마커 URL 이면 {action, data} 를, 아니면 null 을 돌려준다. */
function parseMarker(rawUrl) {
  let url;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }
  if (url.hostname !== MARKER_HOST || !url.hash) {
    return null;
  }
  const data = url.hash.slice(1);
  switch (url.pathname) {
    case "/open":
      return { action: "open", data };
    case "/snapshot":
      return { action: "snapshot", data };
    case "/push":
      return { action: "push", data };
    default:
      return null; // /result 등 앱이 읽을 URL 은 건드리지 않는다
  }
}

function decodeOpenPayload(data) {
  try {
    const payload = JSON.parse(decodeBase64URL(data));
    if (!Array.isArray(payload.urls)) {
      return null;
    }
    // http(s) · file 만 연다
    payload.urls = payload.urls.filter((u) => /^(https?|file):/i.test(u));
    return payload;
  } catch (e) {
    console.warn("LinkGroupz: 마커 디코드 실패", e);
    return null;
  }
}

async function openGroup(markerTabId, payload) {
  const marker = await chrome.tabs.get(markerTabId);
  const tabIds = [];
  for (const [i, url] of payload.urls.entries()) {
    const tab = await chrome.tabs.create({
      windowId: marker.windowId,
      index: marker.index + 1 + i,
      url,
      active: i === 0,
    });
    tabIds.push(tab.id);
  }
  if (tabIds.length > 0) {
    const groupId = await chrome.tabs.group({ tabIds, createProperties: { windowId: marker.windowId } });
    await chrome.tabGroups.update(groupId, {
      title: String(payload.name ?? ""),
      color: COLORS.has(payload.color) ? payload.color : "grey",
      collapsed: false,
    });
  }
  await chrome.tabs.remove(markerTabId);
}

function isMarkerURL(url) {
  try {
    return new URL(url).hostname === MARKER_HOST;
  } catch {
    return false;
  }
}

/** 모든 창의 탭 · 탭 그룹. frontWindowId 창을 첫 번째(앱의 "창 1")로 둔다. 마커 탭은 뺀다. */
async function buildSnapshot(frontWindowId) {
  const windows = await chrome.windows.getAll({ populate: true, windowTypes: ["normal"] });
  windows.sort((a, b) => (b.id === frontWindowId) - (a.id === frontWindowId));
  const groups = await chrome.tabGroups.query({});
  return {
    browser: BROWSER,
    windows: windows.map((w) => ({
      id: w.id,
      focused: w.id === frontWindowId,
      tabs: w.tabs
        .map((t) => ({ title: t.title || "", url: t.url || t.pendingUrl || "", groupId: t.groupId ?? -1 }))
        .filter((t) => !isMarkerURL(t.url)),
    })),
    groups: groups.map((g) => ({ id: g.id, title: g.title || "", color: g.color, windowId: g.windowId })),
  };
}

async function sendSnapshot(markerTabId, nonce) {
  const marker = await chrome.tabs.get(markerTabId);
  // 앱이 맨 앞 창에 마커 탭을 열므로, 마커가 있는 창을 첫 번째로 둔다.
  const payload = { nonce, ...(await buildSnapshot(marker.windowId)) };
  const encoded = encodeBase64URL(JSON.stringify(payload));
  await chrome.tabs.update(markerTabId, { url: `https://${MARKER_HOST}/result#${encoded}` });
}

// MARK: 자동 저장 (Native Messaging)

const NATIVE_HOST = "dev.linkgroupz.snapshot";
let pushTimer = null;
let frontWindowId = chrome.windows.WINDOW_ID_NONE;

function schedulePush() {
  clearTimeout(pushTimer);
  pushTimer = setTimeout(() => pushSnapshot(), 400);
}

/** nonce 는 앱이 요청한 경우(/push)에만 있다. 앱이 자기 요청의 답인지 확인하는 데 쓴다. */
async function pushSnapshot(nonce) {
  try {
    if (frontWindowId === chrome.windows.WINDOW_ID_NONE) {
      const last = await chrome.windows.getLastFocused({ windowTypes: ["normal"] }).catch(() => null);
      frontWindowId = last?.id ?? chrome.windows.WINDOW_ID_NONE;
    }
    const payload = {
      savedAt: Date.now(),
      ...(typeof nonce === "string" ? { nonce } : {}),
      ...(await buildSnapshot(frontWindowId)),
    };
    const reply = await chrome.runtime.sendNativeMessage(NATIVE_HOST, payload);
    if (!reply?.ok) {
      console.warn("LinkGroupz: 앱 저장 실패", reply);
    }
  } catch (e) {
    // 앱이 호스트를 등록하기 전이면 여기로 온다. 앱이 마커 탭 방식으로 대신 받는다.
    console.warn("LinkGroupz: 앱에 탭 목록을 보내지 못했습니다", e?.message ?? e);
  }
}

/** 앱이 연 /push 마커 탭: 그 탭이 있는 창을 맨 앞 창으로 보고 스냅샷을 보낸 뒤, 마커 탭을 닫는다. */
async function pushOnRequest(markerTabId, nonce) {
  try {
    const marker = await chrome.tabs.get(markerTabId);
    frontWindowId = marker.windowId;
    clearTimeout(pushTimer);
    await pushSnapshot(nonce);
  } finally {
    await chrome.tabs.remove(markerTabId).catch(() => {});
  }
}

chrome.windows.onFocusChanged.addListener((windowId) => {
  // 다른 앱으로 전환하면 WINDOW_ID_NONE 이 온다. 마지막 Chrome 창을 맨 앞 창으로 유지한다.
  if (windowId !== chrome.windows.WINDOW_ID_NONE) {
    frontWindowId = windowId;
    schedulePush();
  }
});
chrome.windows.onCreated.addListener(schedulePush);
chrome.windows.onRemoved.addListener(schedulePush);
chrome.tabs.onCreated.addListener(schedulePush);
chrome.tabs.onRemoved.addListener(schedulePush);
chrome.tabs.onMoved.addListener(schedulePush);
chrome.tabs.onAttached.addListener(schedulePush);
chrome.tabs.onDetached.addListener(schedulePush);
chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (changeInfo.url || changeInfo.title || changeInfo.groupId !== undefined) {
    schedulePush();
  }
});
chrome.tabGroups.onCreated.addListener(schedulePush);
chrome.tabGroups.onUpdated.addListener(schedulePush);
chrome.tabGroups.onRemoved.addListener(schedulePush);
chrome.tabGroups.onMoved.addListener(schedulePush);

async function handle(tabId, rawUrl) {
  const marker = parseMarker(rawUrl);
  if (!marker || handledTabs.has(tabId)) {
    return;
  }
  handledTabs.add(tabId);
  try {
    if (marker.action === "open") {
      const payload = decodeOpenPayload(marker.data);
      if (payload) {
        await openGroup(tabId, payload);
      }
    } else if (marker.action === "snapshot") {
      await sendSnapshot(tabId, marker.data);
    } else if (marker.action === "push") {
      await pushOnRequest(tabId, marker.data);
    }
  } catch (e) {
    console.error("LinkGroupz: 처리 실패", marker.action, e);
  } finally {
    setTimeout(() => handledTabs.delete(tabId), 10_000);
  }
}

chrome.webNavigation.onBeforeNavigate.addListener(
  (details) => {
    if (details.frameId === 0) {
      handle(details.tabId, details.url);
    }
  },
  { url: [{ hostEquals: MARKER_HOST }] }
);

// 탐색 이벤트를 놓친 경우(Chrome 이 막 켜지는 중 등)를 대비해 URL 변경과 시작 시점도 본다.
chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (changeInfo.url) {
    handle(tabId, changeInfo.url);
  }
});

async function scanExistingTabs() {
  const tabs = await chrome.tabs.query({});
  for (const tab of tabs) {
    const url = tab.pendingUrl || tab.url;
    if (url) {
      handle(tab.id, url);
    }
  }
}

chrome.runtime.onStartup.addListener(() => {
  scanExistingTabs();
  schedulePush();
});
chrome.runtime.onInstalled.addListener(() => {
  scanExistingTabs();
  schedulePush();
});
