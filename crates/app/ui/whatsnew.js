// 업데이트한 뒤 처음 켤 때 띄우는 '바뀐 점' 창. 설정 → 업데이트 칸의 '바뀐 점 보기'로 다시 연다.

import { fitWindowToContent, html, invoke, render, t, useEffect } from "./lib/ui.js";

const content = (await invoke("whats_new_content")) ?? { title: t("LinkGroupz 바뀐 점"), entries: [] };

function WhatsNew() {
  const close = () => invoke("close_window");

  useEffect(() => {
    const stopFitting = fitWindowToContent();
    const onKey = (event) => {
      if (event.key === "Enter" || event.key === "Escape") {
        close();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      stopFitting();
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  return html`
    <div class="whatsnew-head">
      <img src="app-icon.png" width="44" height="44" alt="" />
      <h1>${content.title}</h1>
    </div>
    <div class="changes">
      ${content.entries.map(
        (entry) => html`<div class="entry" key=${entry.version}>
          <b>${entry.version}</b>
          <ul>
            ${entry.changes.map((change) => html`<li key=${change}>${change}</li>`)}
          </ul>
        </div>`,
      )}
    </div>
    <div class="footer">
      <span class="spacer"></span>
      <button type="button" class="btn primary" onClick=${close}>${t("확인")}</button>
    </div>
  `;
}

render(html`<${WhatsNew} />`, document.getElementById("app"));
