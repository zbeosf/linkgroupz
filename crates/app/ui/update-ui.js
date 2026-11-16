// 업데이트 화면 조각: 관리 창 오른쪽 아래의 버전 표시, 설정 창의 '업데이트' 칸

import { classes, formatTime, html, Icon, invoke, listen, t, useEffect, useState } from "./lib/ui.js";

let latestStatus = await invoke("update_status");
const statusListeners = new Set();

listen("update-changed", (status) => {
  latestStatus = status;
  for (const notify of statusListeners) {
    notify(status);
  }
});

/** 업데이트 상태. 바뀌면 다시 그린다. */
export function useUpdateStatus() {
  const [status, setStatus] = useState(latestStatus);
  useEffect(() => {
    statusListeners.add(setStatus);
    setStatus(latestStatus);
    return () => statusListeners.delete(setStatus);
  }, []);
  return status;
}

/**
 * 관리 창 오른쪽 아래의 설정 버튼. 버전 확인 · 업데이트는 설정 창에서 한다.
 * 새 버전이 있으면 버튼에 점을 찍고, 내려받는 동안에는 진행 상황을 옆에 보여 준다.
 */
export function UpdateBadge() {
  const status = useUpdateStatus();

  // 설정 창의 확인 결과 한 줄은 잠깐 보여 주고 지운다.
  useEffect(() => {
    if (!status.message) {
      return undefined;
    }
    const timer = setTimeout(() => invoke("update_clear_message"), 6000);
    return () => clearTimeout(timer);
  }, [status.message]);

  const available = status.availableVersion;
  return html`<div class="update-badge">
    ${status.phase === "downloading" &&
    html`<span class="progress"><span class="spinner mini"></span>${t("새 버전 내려받는 중…{0}", status.total ? ` ${Math.min(100, Math.floor((status.downloaded * 100) / status.total))}%` : "")}</span>`}
    ${status.phase === "installing" && html`<span class="progress"><span class="spinner mini"></span>${t("설치 준비 중…")}</span>`}
    ${available &&
    status.phase === "idle" &&
    html`<button type="button" class="link-button accent" title=${t("설정에서 업데이트")} onClick=${() => invoke("show_settings")}>${t("새 버전 {0}", available)}</button>`}
    <span class="version">v${status.currentVersion}</span>
    <button type="button" class="link-button" title=${t("설정 (업데이트 · 확장 · 단축키 등)")} onClick=${() => invoke("show_settings")}>
      <${Icon} name="gear" size=${13} />${t("설정")}
    </button>
  </div>`;
}

/** 설정 창의 '업데이트' 칸 */
export function UpdateSettings() {
  const status = useUpdateStatus();
  const idle = status.phase === "idle";

  let statusText = t("새 버전이 나오면 알려 줍니다. 1시간마다 확인합니다.");
  if (!status.autoCheck) {
    statusText = t("새 버전을 스스로 확인하지 않습니다. 오른쪽 위 ↻ 로 직접 확인할 수 있습니다.");
  } else if (status.lastChecked != null) {
    const time = formatTime(new Date(status.lastChecked));
    statusText = status.availableVersion ? t("새 버전 {0} 이 있습니다 ({1} 확인)", status.availableVersion, time) : t("최신 버전입니다 ({0} 확인)", time);
  }

  return html`<section>
    <h3>${t("업데이트")}</h3>
    <div class="card">
      <div class="row">
        <span>${t("현재 버전 {0}", status.currentVersion)}</span>
        <span class="spacer"></span>
        ${status.availableVersion &&
        html`<button type="button" class="btn primary" disabled=${!idle} onClick=${() => invoke("update_install")}>
          ${t("{0} 으로 업데이트", status.availableVersion)}
        </button>`}
        <button type="button" class="btn plain" title=${t("업데이트 확인")} disabled=${!idle || !status.configured} onClick=${() => invoke("update_check")}>
          ${status.phase === "checking" ? html`<span class="spinner mini"></span>` : html`<${Icon} name="arrow-clockwise" size=${15} />`}
        </button>
      </div>
      <div class="divider"></div>
      ${status.configured
        ? html`<div class="row">
              <span>${t("새 버전 자동 확인")}</span>
              <span class="spacer"></span>
              <button
                type="button"
                class=${classes("switch", status.autoCheck && "on")}
                role="switch"
                aria-checked=${status.autoCheck}
                onClick=${() => invoke("update_set_auto_check", { enabled: !status.autoCheck })}
              ></button>
            </div>
            <div class="caption">${status.message ?? statusText}</div>`
        : html`<div class="row">
            <${Icon} name="info" size=${16} class="secondary" />
            <span class="secondary">${t("이 빌드에는 업데이트 주소가 들어 있지 않아 자동 업데이트를 쓸 수 없습니다. 새 버전은 설치 파일로 직접 설치해 주세요.")}</span>
          </div>`}
      <div class="row" style="gap:14px">
        <button type="button" class="btn link" onClick=${() => invoke("show_whats_new")}>${t("바뀐 점 보기")}</button>
        ${status.configured && html`<button type="button" class="btn link" onClick=${() => invoke("open_update_page")}>${t("릴리스 페이지 열기")}</button>`}
      </div>
    </div>
  </section>`;
}
