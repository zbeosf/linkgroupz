// 설정 창

import {
  boot,
  BrowserIcon,
  browserInfo,
  classes,
  Confirm,
  currentProfile,
  GroupIcon,
  html,
  Icon,
  invoke,
  isComposingKey,
  lang,
  render,
  Sheet,
  t,
  useEffect,
  useProfiles,
  useState,
} from "./lib/ui.js";
import { ColorPalette, IconPicker } from "./sheets.js";
import { UpdateSettings } from "./update-ui.js";

/** 새 프로필의 아이콘 (crates/core/src/profiles.rs 의 Profile::default_icon) */
const PROFILE_DEFAULT_ICON = { symbol: "person.2.fill" };

const isMac = boot.platform === "macos";
/** 파일을 보여 주는 프로그램 이름 */
const fileManager = isMac ? "Finder" : t("파일 탐색기");

const MODE_DESCRIPTIONS = {
  auto: t("그룹을 여는 브라우저에 확장이 있으면 이름과 색이 붙은 탭 그룹으로, 없으면 새 창 하나에 모두 엽니다."),
  tabGroup: t("확장 감지와 상관없이 탭 그룹으로 엽니다. 확장이 없으면 접속할 수 없는 페이지(linkgroupz.invalid)가 하나 열립니다."),
  newWindow: t("그룹의 페이지를 그룹에 정한 브라우저(Chrome · Edge · Whale)의 새 창 하나에 모두 엽니다."),
};

function Switch({ on, disabled, onChange }) {
  return html`<button type="button" class=${classes("switch", on && "on")} role="switch" aria-checked=${on} disabled=${disabled} onClick=${() => onChange(!on)}></button>`;
}

/** 프로필: 추가 · 편집(이름 · 아이콘 · 색) · 삭제는 여기서만 한다. 바꾸기는 모든 창의 프로필 버튼으로도 한다. */
function ProfileSection() {
  const state = useProfiles();
  const [name, setName] = useState("");
  const [editing, setEditing] = useState(null); // { id, name, icon, color }
  const [deleting, setDeleting] = useState(null); // 프로필
  const [message, setMessage] = useState(null);

  const counts = (profile) => (profile.groups == null ? t("읽을 수 없음") : t("그룹 {0}개 · 페이지 {1}개", profile.groups, profile.links));
  const run = async (work, done) => {
    try {
      await work();
      setMessage(done ?? null);
      return true;
    } catch (error) {
      setMessage(String(error));
      return false;
    }
  };
  const submitEdit = async () => {
    if (editing.name.trim() && (await run(() => invoke("update_profile", { id: editing.id, name: editing.name, icon: editing.icon, color: editing.color })))) {
      setEditing(null);
    }
  };
  const cancelEdit = () => {
    // 고르다 만 아이콘 이미지를 정리한다.
    invoke("cleanup_group_icons");
    setEditing(null);
    setMessage(null);
  };
  const add = async () => {
    const trimmed = name.trim();
    if (trimmed && (await run(() => invoke("add_profile", { name: trimmed }), t("‘{0}’ 프로필을 만들었습니다. 오른쪽 ‘바꾸기’나 메뉴바 창 · 관리 창의 프로필 버튼으로 바꿉니다.", trimmed)))) {
      setName("");
    }
  };

  return html`<section>
    <h3>${t("프로필")}</h3>
    <div class="card">
      <div class="caption">${t("프로필마다 그룹 · 섹션 · 보관함 · 백업이 따로 있습니다. 관리 창 · 메뉴바 창 · 검색은 지금 프로필의 그룹만 보여 줍니다. 프로필은 메뉴바 창 · 관리 창 위쪽의 프로필 버튼이나 메뉴바 아이콘 오른쪽 클릭 → 프로필에서도 바꿀 수 있습니다.")}</div>
      <div class="column" style="gap:6px">
        ${state.profiles.map(
          (profile) => html`<div class="row profile-item" key=${profile.id}>
            <${GroupIcon} icon=${profile.icon} color=${profile.color} size=${22} />
            <span class="truncate"><b>${profile.name}</b></span>
            <span class="caption">${counts(profile)}${profile.id === state.current ? t(" · 지금 프로필") : ""}</span>
            <span class="spacer"></span>
            <button type="button" class="btn small" disabled=${profile.id === state.current} onClick=${() => run(() => invoke("switch_profile", { id: profile.id }))}>${t("바꾸기")}</button>
            <button type="button" class="btn small" onClick=${() => setEditing({ id: profile.id, name: profile.name, icon: profile.icon, color: profile.color })}>${t("편집")}</button>
            ${!profile.isDefault && html`<button type="button" class="btn small destructive" onClick=${() => setDeleting(profile)}>${t("삭제")}</button>`}
          </div>`,
        )}
      </div>
      <div class="divider"></div>
      <div class="row">
        <input
          class="field"
          style="flex:1 1 auto"
          placeholder=${t("새 프로필 이름 (예: 회사, 개인)")}
          maxlength="30"
          value=${name}
          onInput=${(event) => setName(event.target.value)}
          onKeyDown=${(event) => {
            if (event.key === "Enter" && !isComposingKey(event)) {
              event.preventDefault();
              add();
            }
          }}
        />
        <button type="button" class="btn" disabled=${!name.trim()} onClick=${add}><${Icon} name="plus-bold" size=${12} />${t("추가")}</button>
      </div>
      ${message && html`<div class="caption selectable">${message}</div>`}
    </div>
    ${editing &&
    html`<${Sheet} width=${440} canSubmit=${Boolean(editing.name.trim())} onClose=${cancelEdit} onSubmit=${submitEdit}>
      <h2>${t("프로필 편집")}</h2>
      <label class="column" style="gap:4px">
        <span class="caption">${t("이름")}</span>
        <input class="field" maxlength="30" value=${editing.name} onInput=${(event) => setEditing({ ...editing, name: event.target.value })} />
      </label>
      <div class="column" style="gap:4px">
        <span class="caption">${t("아이콘")}</span>
        <${IconPicker}
          value=${editing.icon}
          color=${editing.color}
          defaultIcon=${PROFILE_DEFAULT_ICON}
          defaultLabel=${t("기본 아이콘으로")}
          onChange=${(icon) => setEditing({ ...editing, icon })}
        />
      </div>
      <div class="column" style="gap:4px">
        <span class="caption">${t("색")}</span>
        <${ColorPalette} value=${editing.color} chromeNote=${false} onChange=${(color) => setEditing({ ...editing, color })} />
      </div>
      ${message && html`<div class="caption">${message}</div>`}
      <div class="actions">
        <button type="button" class="btn" onClick=${cancelEdit}>${t("취소")}</button>
        <button type="button" class="btn primary" disabled=${!editing.name.trim()} onClick=${submitEdit}>${t("저장")}</button>
      </div>
    <//>`}
    ${deleting &&
    html`<${Confirm}
      title=${t("‘{0}’ 프로필을 지울까요?", deleting.name)}
      message=${t("{0} 목록에서 사라집니다.{1} 데이터는 지우지 않고 백업 폴더(deleted-profiles)로 옮겨 두므로, 필요하면 그 안의 data.json 을 ‘가져오기’로 되살릴 수 있습니다 (그룹 아이콘 이미지도 함께 돌아옵니다).", deleting.groups != null ? t("그룹 {0}개 · 페이지 {1}개가", deleting.groups, deleting.links) : t("이 프로필의 그룹이"), deleting.id === state.current ? t(" 지금 프로필이라 기본 프로필로 바꾼 뒤 지웁니다.") : "")}
      confirmLabel=${t("삭제")}
      onConfirm=${async () => {
        const profile = deleting;
        setDeleting(null);
        await run(() => invoke("delete_profile", { id: profile.id }), t("‘{0}’ 프로필을 지웠습니다.", profile.name));
      }}
      onCancel=${() => setDeleting(null)}
    />`}
  </section>`;
}

/** 로그인할 때 자동으로 실행 */
function StartupSection() {
  const [launch, setLaunch] = useState(null);
  const [error, setError] = useState(null);

  // 시스템 설정에서 직접 바꿨을 수 있어 창을 열 때 · 다시 앞으로 올 때마다 다시 읽는다.
  useEffect(() => {
    const refresh = () => invoke("launch_at_login").then(setLaunch);
    refresh();
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, []);

  const change = async (enabled) => {
    try {
      setLaunch(await invoke("set_launch_at_login", { enabled }));
      setError(null);
    } catch (message) {
      setError(String(message));
      setLaunch(await invoke("launch_at_login"));
    }
  };

  let note = isMac
    ? t("Mac 을 재부팅하거나 다시 로그인하면 메뉴바에 LinkGroupz 가 바로 뜹니다.")
    : t("Windows 에 로그인하면 작업 표시줄 트레이에 LinkGroupz 가 바로 뜹니다.");
  let action = null;
  if (launch && !launch.installedLocation) {
    note = t("DMG 안에서 실행 중이라 켤 수 없습니다. 응용 프로그램 폴더로 옮긴 뒤 다시 켜 주세요.");
  } else if (launch?.state === "requiresApproval") {
    note = t("시스템 설정 → 일반 → 로그인 항목에서 LinkGroupz 를 허용해야 자동으로 켜집니다.");
    action = html`<button type="button" class="btn small" onClick=${() => invoke("open_login_items_settings")}>${t("로그인 항목 열기")}</button>`;
  }

  return html`<section>
    <h3>${t("시작")}</h3>
    <div class="card">
      <div class="row">
        <span>${t("로그인할 때 자동으로 실행")}</span>
        <span class="spacer"></span>
        <${Switch} on=${launch != null && launch.state !== "disabled"} disabled=${!launch?.installedLocation} onChange=${change} />
      </div>
      <div class="row caption">
        ${error ? html`<span class="danger-text">${error}</span>` : html`<span>${note}</span>`}
        <span class="spacer"></span>
        ${action}
      </div>
      <div class="divider"></div>
      <${ManagerOnLaunchRow} />
      ${isMac && html`<div class="divider"></div><${DockRow} />`}
    </div>
  </section>`;
}

/** 단축키 글자("Control+Alt+KeyK")를 보기 좋게: macOS 는 ⌃⌥K, Windows 는 Ctrl + Alt + K */
function shortcutLabel(shortcut) {
  const parts = shortcut.split("+");
  const code = parts.pop();
  const names = isMac
    ? { Control: "⌃", Alt: "⌥", Shift: "⇧", Super: "⌘" }
    : { Control: "Ctrl", Alt: "Alt", Shift: "Shift", Super: "Win" };
  const keys = { Space: "Space", ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→", Backquote: "`", Minus: "-", Equal: "=", Comma: ",", Period: ".", Slash: "/", Semicolon: ";", Quote: "'", BracketLeft: "[", BracketRight: "]", Backslash: "\\" };
  const key = keys[code] ?? code.replace(/^Key|^Digit/, "");
  const modifiers = ["Control", "Alt", "Shift", "Super"].filter((name) => parts.includes(name)).map((name) => names[name]);
  return isMac ? modifiers.join("") + key : [...modifiers, key].join(" + ");
}

/** 누른 키를 단축키 글자로. 보조 키만 누른 상태면 null */
function shortcutFromEvent(event) {
  // 글자는 event.key 가 아니라 자리(event.code)로 본다. ⌥ 를 누르면 key 는 특수 문자나 "Dead" 가 되지만 code 는 그대로다.
  if (["Control", "Alt", "Shift", "Meta", "OS"].includes(event.key) || !event.code || event.code === "Unidentified") {
    return null;
  }
  const modifiers = [event.ctrlKey && "Control", event.altKey && "Alt", event.shiftKey && "Shift", event.metaKey && "Super"].filter(Boolean);
  return [...modifiers, event.code].join("+");
}

/** 메뉴바(트레이) 창을 여닫는 단축키 */
function ShortcutSection() {
  const [status, setStatus] = useState(null);
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState(null);

  useEffect(() => {
    invoke("popover_shortcut").then(setStatus);
  }, []);

  // 받는 동안: 누른 키가 다른 데로 가지 않게 막고, 지금 단축키는 잠시 듣지 않는다.
  useEffect(() => {
    if (!recording) {
      return undefined;
    }
    invoke("pause_popover_shortcut", { paused: true });
    const stop = () => setRecording(false);
    const onKey = async (event) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape" && !event.ctrlKey && !event.altKey && !event.metaKey) {
        stop();
        return;
      }
      const shortcut = shortcutFromEvent(event);
      if (!shortcut) {
        return;
      }
      try {
        setStatus(await invoke("set_popover_shortcut", { shortcut }));
        setError(null);
        stop();
      } catch (message) {
        setError(String(message));
      }
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", stop);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("blur", stop);
      invoke("pause_popover_shortcut", { paused: false }).then(() => invoke("popover_shortcut").then(setStatus));
    };
  }, [recording]);

  const clear = async () => {
    setStatus(await invoke("set_popover_shortcut", { shortcut: null }));
    setError(null);
  };

  const shortcut = status?.shortcut;
  let note = t("다른 프로그램을 쓰는 중에도 이 단축키로 {0} 창을 엽니다. 열리면 바로 검색할 수 있고, 열린 채로 한 번 더 누르면 관리 창이 열립니다.", isMac ? t("메뉴바") : t("트레이"));
  if (recording) {
    note = t("쓸 단축키를 누르세요. Ctrl · Alt{0} 중 하나와 함께 눌러야 합니다. Esc 로 그만둡니다.", isMac ? "(⌥) · ⌘" : t(" · Windows 키"));
  } else if (shortcut && status.active === false) {
    note = t("다른 프로그램이 이 단축키를 쓰고 있어 지금은 동작하지 않습니다. 다른 단축키로 바꿔 주세요.");
  }

  return html`<section>
    <h3>${t("단축키")}</h3>
    <div class="card">
      <div class="row">
        <span>${t("{0} 창 열기 · 닫기", isMac ? t("메뉴바") : t("트레이"))}</span>
        <span class="spacer"></span>
        <button type="button" class=${classes("btn", recording && "primary")} style="min-width:96px" disabled=${!status} onClick=${() => setRecording(!recording)}>
          ${recording ? t("누르세요…") : shortcut ? shortcutLabel(shortcut) : t("없음")}
        </button>
        ${shortcut && !recording && html`<button type="button" class="btn" style="flex:none" onClick=${clear}>${t("지우기")}</button>`}
      </div>
      <div class="caption">${error ? html`<span class="danger-text">${error}</span>` : note}</div>
    </div>
  </section>`;
}

/** 앱을 켤 때 관리 창 열기 */
function ManagerOnLaunchRow() {
  const [show, setShow] = useState(boot.showManagerOnLaunch);
  const change = (value) => {
    setShow(value);
    invoke("set_show_manager_on_launch", { show: value });
  };
  const place = isMac ? t("메뉴바") : t("트레이");
  return html`<div class="row">
      <span>${t("앱을 켤 때 여는 창")}</span>
      <span class="spacer"></span>
      <div class="segmented fit">
        <button type="button" class=${classes(!show && "selected")} onClick=${() => change(false)}>${t("{0} 창 (기본)", place)}</button>
        <button type="button" class=${classes(show && "selected")} onClick=${() => change(true)}>${t("관리 창")}</button>
      </div>
    </div>
    <div class="caption">${t("앱을 직접 켤 때 {0} 창이나 관리 창을 엽니다. 로그인할 때 자동으로 켜진 경우에는 아무 창도 열지 않습니다. 처음 켤 때는 시작 안내와 관리 창이 열립니다.", place)}</div>`;
}

/** Dock 에 아이콘 표시 (macOS) */
function DockRow() {
  const [show, setShow] = useState(boot.showDockIcon);
  const change = (value) => {
    setShow(value);
    invoke("set_show_dock_icon", { show: value });
  };
  return html`<div class="row">
      <span>${t("Dock 에 아이콘 표시")}</span>
      <span class="spacer"></span>
      <${Switch} on=${show} onChange=${change} />
    </div>
    <div class="caption">${t("켜 두면 Dock 아이콘이나 ⌘Tab 으로 LinkGroupz 로 바로 돌아오고, Dock 아이콘을 누르면 관리 창이 열립니다. 끄면 메뉴바 아이콘으로만 씁니다.")}</div>`;
}

/** 그룹 열기 방식 + 브라우저마다 확장 감지 여부 */
function OpenModeSection({ states, onRefresh }) {
  const [mode, setMode] = useState(boot.groupOpenMode);
  const change = (event) => {
    setMode(event.target.value);
    invoke("set_group_open_mode", { mode: event.target.value });
  };
  return html`<section>
    <h3>${t("그룹 열기")}</h3>
    <div class="card">
      <div class="row">
        <span>${t("방식")}</span>
        <span class="spacer"></span>
        <select class="field" value=${mode} onChange=${change}>
          ${boot.openModes.map((item) => html`<option key=${item.id} value=${item.id}>${item.name}</option>`)}
        </select>
      </div>
      <div class="caption">${MODE_DESCRIPTIONS[mode]}</div>
      <div class="divider"></div>
      ${boot.browsers.map((browser) => {
        const state = states?.find((item) => item.browser === browser.id);
        return html`<div class="row" key=${browser.id}>
          <${BrowserIcon} browser=${browser.id} size=${16} />
          <span style="width:56px">${browser.name}</span>
          ${!state
            ? html`<span class="secondary">${t("확인 중…")}</span>`
            : !state.installed
              ? html`<span class="secondary">${t("설치돼 있지 않음")}</span>`
              : state.found
                ? html`<${Icon} name="check-circle-fill" size=${16} class="ok" />
                    <span title=${state.found.path}>
                      ${t("확장 감지됨 ({0})", state.found.profile)}
                    </span>`
                : html`<${Icon} name="x-circle" size=${16} class="secondary" /><span>${t("확장 감지 안 됨")}</span>`}
        </div>`;
      })}
      <div class="row">
        <span class="caption">${t("새 그룹은 Chrome 에서 엽니다. 그룹마다 편집 화면이나 오른쪽 클릭 → ‘브라우저’에서 바꿀 수 있습니다.")}</span>
        <span class="spacer"></span>
        <button type="button" class="btn" style="flex:none" onClick=${onRefresh}>${t("다시 확인")}</button>
      </div>
    </div>
  </section>`;
}

/** 브라우저에 확장을 넣는 방법 */
function ExtensionSection() {
  const [guideBrowser, setGuideBrowser] = useState(boot.browsers[0].id);
  const [folder, setFolder] = useState(null);
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    invoke("extension_folder").then(setFolder);
  }, []);
  const browser = browserInfo(guideBrowser);
  const copy = async () => {
    await invoke("copy_text", { text: browser.extensionsPage });
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };
  return html`<section>
    <h3>${t("확장 설치")}</h3>
    <div class="card">
      <div class="segmented">
        ${boot.browsers.map(
          (item) => html`<button type="button" key=${item.id} class=${classes(guideBrowser === item.id && "selected")} onClick=${() => setGuideBrowser(item.id)}>
            ${item.name}
          </button>`,
        )}
      </div>
      <div class="column" style="gap:4px">
        <span>${t("1. {0} 주소창에 {1} 입력", browser.name, browser.extensionsPage)}</span>
        <span>${guideBrowser === "edge" ? t("2. 왼쪽 메뉴(또는 아래쪽)의 ‘개발자 모드’ 켜기") : t("2. 오른쪽 위 ‘개발자 모드’ 켜기")}</span>
        <span>${t("3. ‘{0}’ → 아래 폴더 선택", browser.loadUnpackedLabel)}</span>
        <span>${t("4. 위에서 ‘다시 확인’ — 자동 모드면 바로 탭 그룹으로 엽니다")}</span>
      </div>
      <div class="row">
        <button type="button" class="btn" disabled=${!folder} onClick=${() => invoke("reveal_extension_folder")}>${t("확장 폴더 {0} 에서 보기", fileManager)}</button>
        <button type="button" class="btn" onClick=${copy}>${copied ? t("복사했습니다") : t("{0} 복사", browser.extensionsPage)}</button>
      </div>
      ${folder && html`<div class="caption selectable">${folder}</div>`}
    </div>
  </section>`;
}

/** 미리보기에서 로그인한 정보 */
function PreviewLoginSection() {
  /** 보관소를 읽어야 알 수 있어 버튼을 누르기 전에는 모른다 (설정을 열 때마다 키체인 암호 창이 뜨지 않게) */
  const [count, setCount] = useState(undefined);
  const [checking, setChecking] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const check = async () => {
    setChecking(true);
    setCount(await invoke("preview_site_count"));
    setChecking(false);
  };
  const clear = async () => {
    setConfirming(false);
    await invoke("clear_preview_logins");
    setCount(isMac ? 0 : undefined);
  };
  return html`<section>
    <h3>${t("미리보기 로그인")}</h3>
    <div class="card">
      <div class="caption">
        ${isMac
          ? t("미리보기에서 로그인한 쿠키는 키체인(LinkGroupz 미리보기 로그인 쿠키)에 암호화해 보관합니다. 쿠키가 만료되면 미리보기에서 다시 로그인해야 합니다.")
          : t("미리보기에서 로그인한 쿠키는 미리보기 전용 브라우저 프로필(앱 데이터 폴더의 preview-profile)에 Windows 가 암호화해 보관합니다. 쿠키가 만료되면 미리보기에서 다시 로그인해야 합니다.")}
      </div>
      <div class="row">
        ${isMac &&
        (count === undefined || count === null
          ? html`<button
              type="button"
              class="btn"
              disabled=${checking}
              title=${t("키체인을 읽어 저장된 사이트 수를 봅니다. macOS 가 키체인 접근을 물을 수 있습니다.")}
              onClick=${check}
            >${checking ? t("확인 중…") : t("저장된 로그인 확인")}</button>`
          : html`<span>${count === 0 ? t("저장된 로그인 없음") : t("사이트 {0}곳 저장됨", count)}</span>`)}
        <span class="spacer"></span>
        <button type="button" class="btn" disabled=${count === 0} onClick=${() => setConfirming(true)}>${t("미리보기 로그인 정보 지우기")}</button>
      </div>
    </div>
    ${confirming &&
    html`<${Confirm}
      title=${t("미리보기 로그인 정보를 지울까요?")}
      message=${t("저장된 미리보기 쿠키를 모두 지웁니다. 다음 미리보기부터 다시 로그인해야 합니다.")}
      confirmLabel=${t("지우기")}
      onConfirm=${clear}
      onCancel=${() => setConfirming(false)}
    />`}
  </section>`;
}

/** 오늘 날짜 ("2026-11-12", 이 컴퓨터 시각) */
function todayText() {
  const now = new Date();
  const pad = (number) => String(number).padStart(2, "0");
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

/** 보관 기간(days 일, 오늘 포함)을 넘는 백업 수. 끔(0) · 무제한이면 0 */
function expiredCount(backups, days, keepAll) {
  if (days === 0 || days === keepAll) {
    return 0;
  }
  const oldest = new Date(`${todayText()}T00:00:00`);
  oldest.setDate(oldest.getDate() - (days - 1));
  return backups.filter((backup) => new Date(`${backup.date}T00:00:00`) < oldest).length;
}

/** 데이터 파일 · 자동 백업 */
function DataSection() {
  const [state, setState] = useState(null);
  const [confirm, setConfirm] = useState(null); // { kind: "days", days, count } | { kind: "restore", date } | { kind: "import", candidate }
  const [message, setMessage] = useState(null);
  const profiles = useProfiles();
  const profile = currentProfile(profiles);
  const reload = () => invoke("backups").then(setState);
  // 내보내기 · 가져오기 · 백업은 지금 프로필 것이라, 프로필을 바꾸면 다시 읽는다.
  useEffect(() => {
    reload();
    setMessage(null);
  }, [profiles.current]);

  const chooseDays = (days) => {
    const count = expiredCount(state.backups, days, state.keepAll);
    if (count > 0) {
      setConfirm({ kind: "days", days, count });
    } else {
      invoke("set_backup_days", { days }).then(setState);
    }
  };
  const restore = async (date) => {
    setConfirm(null);
    try {
      await invoke("restore_backup", { date });
      setMessage(t("{0} 백업으로 되돌렸습니다. 되돌리기 전 데이터는 백업 폴더에 남겨 두었습니다.", date));
      reload();
    } catch (error) {
      setMessage(String(error));
    }
  };

  const exportData = async () => {
    try {
      const saved = await invoke("export_data");
      if (saved) {
        setMessage(t("내보냈습니다: {0}", saved));
      }
    } catch (error) {
      setMessage(String(error));
    }
  };
  const chooseImport = async () => {
    try {
      const candidate = await invoke("choose_import_file");
      if (candidate) {
        setConfirm({ kind: "import", candidate });
      }
    } catch (error) {
      setMessage(String(error));
    }
  };
  const importData = async (mode) => {
    const { candidate } = confirm;
    setConfirm(null);
    try {
      await invoke("import_data", { path: candidate.path, mode });
      setMessage(
        mode === "merge"
          ? t("‘{0}’ 의 그룹 {1}개를 지금 목록 끝에 더했습니다.", candidate.fileName, candidate.summary.groups)
          : t("‘{0}’ 으로 바꿨습니다. 바꾸기 전 데이터는 백업 폴더에 남겨 두었습니다.", candidate.fileName),
      );
      reload();
    } catch (error) {
      setMessage(String(error));
    }
  };

  const dayLabel = (days) => (days === 0 ? t("끔") : days === state?.keepAll ? t("무제한") : t("{0}일치{1}", days, days === 7 ? t(" (기본)") : ""));
  return html`<section>
    <h3>${t("데이터 · 백업{0}", profiles.profiles.length > 1 ? t(" — ‘{0}’ 프로필", profile?.name) : "")}</h3>
    <div class="card">
      <div class="row">
        <span>${t("그룹 · 페이지 · 섹션 · 보관함")}</span>
        <span class="spacer"></span>
        <button type="button" class="btn" title=${t("지금 데이터를 파일 하나로 저장합니다 (다른 컴퓨터로 옮기거나 따로 보관할 때)")} onClick=${exportData}>${t("내보내기…")}</button>
        <button type="button" class="btn" title=${t("내보낸 파일이나 원본 LinkGroups 의 data.json 을 가져옵니다")} onClick=${chooseImport}>${t("가져오기…")}</button>
      </div>
      <div class="caption">${t("내보낸 파일은 다른 컴퓨터의 LinkGroupz 에서 가져올 수 있습니다. 내 이미지로 바꾼 그룹 아이콘도 함께 들어갑니다 (하나에 수십 KB). 원본 LinkGroups 의 data.json 도 가져올 수 있습니다.")}</div>
      <div class="divider"></div>
      <div class="row">
        <span>${t("자동 백업")}</span>
        <span class="spacer"></span>
        <select class="field" value=${state?.days ?? 7} disabled=${!state} onChange=${(event) => chooseDays(Number(event.target.value))}>
          ${(state?.choices ?? [0, 3, 7, 14, 30]).map((days) => html`<option key=${days} value=${days}>${dayLabel(days)}</option>`)}
        </select>
      </div>
      <div class="caption">
        ${t("하루에 한 번 그날 처음 상태의 그룹 · 페이지 · 보관함을 백업해 두고, 고른 기간이 지난 백업은 지웁니다 (무제한이면 지우지 않습니다). 잘못 지우거나 바꿨을 때 아래에서 그날 상태로 되돌릴 수 있습니다. 내 이미지로 바꾼 그룹 아이콘도 함께 백업합니다.")}
      </div>
      ${state?.backups.length > 0 &&
      html`<div class="column" style="gap:4px">
        ${state.backups.map(
          (backup) => html`<div class="row" key=${backup.date}>
            <span>${backup.date}${backup.date === todayText() ? t(" (오늘)") : ""}</span>
            <span class="caption">${backup.groupCount != null ? t("그룹 {0}개", backup.groupCount) : t("읽을 수 없음")}</span>
            <span class="spacer"></span>
            <button type="button" class="btn small" disabled=${backup.groupCount == null} onClick=${() => setConfirm({ kind: "restore", date: backup.date })}>${t("되돌리기")}</button>
          </div>`,
        )}
      </div>`}
      ${message && html`<div class="caption selectable">${message}</div>`}
    </div>
    ${confirm?.kind === "import" &&
    html`<${Sheet} width=${460} onClose=${() => setConfirm(null)}>
      <h2>${t("‘{0}’ 가져오기", confirm.candidate.fileName)}</h2>
      <div class="secondary">
        ${t("그룹 {0}개 · 페이지 {1}개{2}{3}{4}", confirm.candidate.summary.groups, confirm.candidate.summary.links, confirm.candidate.summary.sections > 0 ? t(" · 섹션 {0}개", confirm.candidate.summary.sections) : "", confirm.candidate.summary.archived > 0 ? t(" · 보관함 {0}개", confirm.candidate.summary.archived) : "", confirm.candidate.summary.iconImages > 0 ? t(" · 아이콘 이미지 {0}개", confirm.candidate.summary.iconImages) : "")}
      </div>
      <div class="column" style="gap:4px">
        <span><b>${t("지금 목록에 더하기")}</b> ${t("— 지금 그룹은 그대로 두고 가져온 그룹을 끝에 더합니다.")}</span>
        <span><b>${t("이것으로 바꾸기")}</b> ${t("— 지금 그룹을 모두 가져온 내용으로 바꿉니다. 바꾸기 전 데이터는 백업 폴더에 따로 남깁니다.")}</span>
      </div>
      <div class="actions">
        <button type="button" class="btn" onClick=${() => setConfirm(null)}>${t("취소")}</button>
        <button type="button" class="btn destructive" onClick=${() => importData("replace")}>${t("이것으로 바꾸기")}</button>
        <button type="button" class="btn primary" onClick=${() => importData("merge")}>${t("지금 목록에 더하기")}</button>
      </div>
    <//>`}
    ${confirm?.kind === "days" &&
    html`<${Confirm}
      title=${t("보관 기간을 {0}로 줄일까요?", dayLabel(confirm.days))}
      message=${t("기간이 지난 백업 {0}개가 지워집니다.", confirm.count)}
      confirmLabel=${t("줄이기")}
      onConfirm=${() => {
        setConfirm(null);
        invoke("set_backup_days", { days: confirm.days }).then(setState);
      }}
      onCancel=${() => setConfirm(null)}
    />`}
    ${confirm?.kind === "restore" &&
    html`<${Confirm}
      title=${t("{0} 백업으로 되돌릴까요?", confirm.date)}
      message=${t("지금 그룹 · 페이지 · 보관함이 그날 상태로 바뀝니다. 지금 데이터는 백업 폴더에 따로 남겨 둡니다.")}
      confirmLabel=${t("되돌리기")}
      onConfirm=${() => restore(confirm.date)}
      onCancel=${() => setConfirm(null)}
    />`}
  </section>`;
}

/** 화면 모드: 시스템 · 라이트 · 다크. 앱 전체 모양을 바꿔 모든 창이 함께 바뀐다 (commands::set_theme). */
function ThemeSection() {
  const [choice, setChoice] = useState(boot.themeSetting ?? null);
  const change = (theme) => {
    setChoice(theme);
    invoke("set_theme", { theme });
  };
  return html`<section>
    <h3>${t("화면 모드")}</h3>
    <div class="card">
      <div class="segmented">
        <button type="button" class=${classes(choice == null && "selected")} onClick=${() => change(null)}>${t("시스템")}</button>
        <button type="button" class=${classes(choice === "light" && "selected")} onClick=${() => change("light")}>${t("라이트")}</button>
        <button type="button" class=${classes(choice === "dark" && "selected")} onClick=${() => change("dark")}>${t("다크")}</button>
      </div>
      <div class="caption">${t("시스템은 {0} 의 화면 모드를 따릅니다.", isMac ? "macOS" : "Windows")}</div>
    </div>
  </section>`;
}

/** 화면 언어: 시스템 · 한국어 · English. 바꾸면 모든 창이 다시 그려진다 (crates/core/src/i18n.rs). */
function LanguageSection() {
  const [choice, setChoice] = useState(boot.languageSetting ?? null);
  const change = (language) => {
    setChoice(language);
    invoke("set_language", { language });
  };
  // 언어 이름은 늘 그 언어로 적는다 (다른 언어로 바꿔 둔 사람도 찾을 수 있게).
  return html`<section>
    <h3>${lang === "en" ? "Language" : "언어 · Language"}</h3>
    <div class="card">
      <div class="segmented">
        <button type="button" class=${classes(choice == null && "selected")} onClick=${() => change(null)}>${t("시스템")}</button>
        <button type="button" class=${classes(choice === "ko" && "selected")} onClick=${() => change("ko")}>한국어</button>
        <button type="button" class=${classes(choice === "en" && "selected")} onClick=${() => change("en")}>English</button>
      </div>
      <div class="caption">${t("시스템은 {0} 의 언어를 따릅니다 (한국어가 아니면 영어).", isMac ? "macOS" : "Windows")}</div>
    </div>
  </section>`;
}

function Settings() {
  const [states, setStates] = useState(null);
  // 확장을 방금 설치했으면 탭 목록 자동 저장 연결도 바로 등록된다 (앱이 해 준다).
  const refresh = () => invoke("extension_states").then(setStates);
  useEffect(() => {
    refresh();
  }, []);

  return html`
    <${ProfileSection} />
    <${StartupSection} />
    <${ShortcutSection} />
    <${UpdateSettings} />
    <${OpenModeSection} states=${states} onRefresh=${refresh} />
    <${ExtensionSection} />
    <${PreviewLoginSection} />
    <${DataSection} />
    <${ThemeSection} />
    <${LanguageSection} />
    <section>
      <h3>${t("오픈소스 라이선스")}</h3>
      <div class="card">
        <div class="row">
          <span class="caption">${t("LinkGroupz 에 들어 있는 외부 코드의 저작권 · 라이선스 고지입니다.")}</span>
          <span class="spacer"></span>
          <button type="button" class="btn" style="flex:none" disabled=${!boot.hasLicenses} onClick=${() => invoke("open_licenses")}>${t("보기")}</button>
        </div>
      </div>
    </section>
  `;
}

render(html`<${Settings} />`, document.getElementById("app"));
