// 설치 후 처음 실행할 때 띄우는 세팅 안내. 메뉴바 창의 "안내" 로 다시 열 수 있다.

import { boot, fitWindowToContent, html, Icon, invoke, render, t, useEffect, useState } from "./lib/ui.js";

const isMac = boot.platform === "macos";
const fileManager = isMac ? "Finder" : t("파일 탐색기");

/** 메뉴바 아이콘 모양 (그룹 카드 세 장) */
function TrayGlyph() {
  return html`<svg width="15" height="13" viewBox="0 0 18 16" style="vertical-align:-2px" fill="currentColor" aria-label=${t("LinkGroupz 아이콘")}>
    <rect x="1" y="1.25" width="3.5" height="3.5" rx="1.1" />
    <rect x="6.25" y="1.75" width="10.75" height="2.5" rx="1.25" />
    <rect x="1" y="6.25" width="3.5" height="3.5" rx="1.1" />
    <rect x="6.25" y="6.75" width="8" height="2.5" rx="1.25" />
    <rect x="1" y="11.25" width="3.5" height="3.5" rx="1.1" />
    <rect x="6.25" y="11.75" width="10.75" height="2.5" rx="1.25" />
  </svg>`;
}

function Step({ number, title, children }) {
  return html`<div class="step">
    <span class="number">${number}</span>
    <div class="content"><b>${title}</b>${children}</div>
  </div>`;
}

function Welcome() {
  const [detected, setDetected] = useState(null);
  const [folder, setFolder] = useState(null);
  const [copied, setCopied] = useState(false);

  // 확장을 로드하면 '확장 설치됨'으로 저절로 바뀌도록 2초마다 다시 확인한다.
  useEffect(() => {
    const refresh = async () => {
      const states = await invoke("extension_states");
      setDetected(states.map((state) => state.found).find(Boolean) ?? null);
    };
    refresh();
    invoke("extension_folder").then(setFolder);
    const timer = setInterval(refresh, 2000);
    return () => clearInterval(timer);
  }, []);

  // 내용 높이에 창을 맞춘다 (확장 감지 결과 등으로 내용이 바뀌면 다시 맞춘다).
  useEffect(() => fitWindowToContent(), []);

  const copy = async () => {
    await invoke("copy_text", { text: "chrome://extensions" });
    setCopied(true);
  };
  const close = () => invoke("close_window");

  return html`
    <div class="welcome-head">
      <img src="app-icon.png" width="52" height="52" alt="" />
      <div class="column" style="gap:2px">
        <h1>${t("LinkGroupz 시작하기")}</h1>
        <span class="secondary">${t("버전 {0} · 자주 여는 웹 페이지를 그룹으로 모아 Chrome · Edge · Whale 에서 한 번에 엽니다", boot.version)}</span>
      </div>
    </div>

    ${isMac
      ? html`<${Step} number=${1} title=${t("메뉴바의 아이콘에서 시작합니다")}>
          <span>
            ${t("화면 위 메뉴바의")} <${TrayGlyph} /> ${t("아이콘(카드 세 장)을 누르면 그룹을 검색하고 바로 열 수 있습니다. 그룹과 페이지 편집은 아이콘 창 아래\n            ‘관리’ 버튼에서 합니다.")}
          </span>
        <//>`
      : html`<${Step} number=${1} title=${t("작업 표시줄 트레이의 아이콘에서 시작합니다")}>
          <span>
            ${t("화면 오른쪽 아래 트레이의 LinkGroupz 아이콘(카드 세 장)을 누르면 그룹을 검색하고 바로 열 수 있습니다. 아이콘이 안 보이면 ∧ 를 눌러\n            숨겨진 아이콘에서 찾습니다. 그룹과 페이지 편집은 아이콘 창 아래 ‘관리’ 버튼에서 합니다.")}
          </span>
        <//>`}

    <${Step} number=${2} title=${isMac ? t("Chrome 확장 설치 (권장)") : t("Chrome 확장 설치 (열린 탭을 가져오려면 필요)")}>
      <span>
        ${isMac
          ? t("설치하면 그룹을 이름 · 색이 붙은 탭 그룹으로 열고, 열린 탭 · 탭 그룹을 브라우저 창을 건드리지 않고 가져옵니다.")
          : t("설치하면 그룹을 이름 · 색이 붙은 탭 그룹으로 열고, 브라우저에 열린 탭 · 탭 그룹을 가져올 수 있습니다.")}
      </span>
      <div class="column" style="gap:3px">
        <span>${t("① Chrome 주소창에")} <b>chrome://extensions</b> ${t("입력 (아래 버튼으로 복사)")}</span>
        <span>${t("② 오른쪽 위")} <b>${t("개발자 모드")}</b> ${t("켜기")}</span>
        <span>③ <b>${t("압축해제된 확장 프로그램 로드")}</b> ${t("→ 아래 ‘확장 폴더’ 선택")}</span>
        <span class="secondary">${t("Edge 도 쓰면")} <b>edge://extensions</b> → <b>${t("개발자 모드")}</b> → <b>${t("압축을 푼 확장 로드")}</b>${t(", Whale 은")} <b>whale://extensions</b> → <b>${t("개발자 모드")}</b> → <b>${t("압축해제 된 확장앱 설치")}</b> ${t("로 같은 폴더를 넣습니다.")}</span>
      </div>
      <div class="row">
        <button type="button" class="btn" onClick=${copy}>${copied ? t("복사했습니다") : t("chrome://extensions 복사")}</button>
        <button type="button" class="btn" disabled=${!folder} onClick=${() => invoke("reveal_extension_folder")}>${t("확장 폴더 {0} 에서 보기", fileManager)}</button>
        <span class="spacer"></span>
        <span class="row" style="gap:4px;font-weight:600" title=${detected?.path ?? t("확장을 로드하면 자동으로 바뀝니다")}>
          <${Icon} name=${detected ? "check-circle-fill" : "hourglass"} size=${16} class=${detected ? "ok" : "secondary"} />
          ${detected ? t("확장 설치됨") : t("확장을 기다리는 중")}
        </span>
      </div>
      ${folder && html`<span class="caption selectable">${t("확장 폴더: {0}", folder)}</span>`}
    <//>

    ${isMac &&
    html`<${Step} number=${3} title=${t("macOS 가 물어보면 허용해 주세요")}>
      <div class="column" style="gap:3px">
        <span>• <b>${t("‘Google Chrome(또는 Microsoft Edge · Whale)을 제어하려고 합니다’")}</b> ${t("— 열린 탭 가져오기 (확장이 없을 때)")}</span>
        <span>• <b>${t("키체인 접근")}</b> ${t("— 미리보기 로그인 정보를 암호화해 보관할 때 (‘항상 허용’)")}</span>
      </div>
    <//>`}

    <div class="footer">
      <button type="button" class="btn" disabled=${!boot.hasGuide} onClick=${() => invoke("open_guide")}>${t("사용법 보기")}</button>
      <span class="spacer"></span>
      <button type="button" class="btn" onClick=${close}>${t("닫기")}</button>
      <button
        type="button"
        class="btn primary"
        onClick=${async () => {
          await invoke("show_manager");
          close();
        }}
      >${t("관리 창 열기")}</button>
    </div>
  `;
}

render(html`<${Welcome} />`, document.getElementById("app"));
