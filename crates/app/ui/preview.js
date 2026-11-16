// 미리보기 창 위쪽의 띠.
//
// - 작은 미리보기: 제목 · 주소, '크게 열기', '<브라우저> 에서 열기'
// - 크게 연 창: 뒤로 · 앞으로 · 새로고침, 주소, 설치된 브라우저마다 '<브라우저> 에서 열기'

import { boot, BrowserIcon, browserInfo, classes, html, Icon, invoke, listen, render, t, useEffect, useState } from "./lib/ui.js";

const initial = await invoke("preview_info");

function PreviewBar() {
  const [info, setInfo] = useState(initial);

  useEffect(
    () =>
      listen("preview-info", (updated) => {
        // 다른 미리보기 창의 소식은 넘긴다.
        setInfo((current) => (!current || current.label === updated.label ? updated : current));
      }),
    [],
  );

  if (!info) {
    return null;
  }
  const title = info.title || info.fallbackTitle;
  const status = info.slow
    ? html`<span class="slow">${t("응답이 늦어지고 있습니다. 네트워크 · VPN 연결을 확인해 주세요.")}</span>`
    : info.loading && html`<span class="spinner mini"></span>`;
  const openIn = (browser) => invoke("preview_open_in_browser", { browser });

  if (info.compact) {
    const name = browserInfo(info.browser).name;
    return html`<div class="bar compact">
      <div class="row">
        <div class="titles" style="flex:1 1 auto">
          <b class="truncate">${title}</b>
          <span class="caption truncate">${info.url}</span>
        </div>
        ${status}
      </div>
      <div class="row">
        <button type="button" class="btn small" title=${t("원래 크기 창으로 열기 — 로그인이 필요하면 여기서 로그인하세요")} onClick=${() => invoke("preview_expand")}>
          <${Icon} name="arrows-out" size=${13} />${t("크게 열기")}
        </button>
        <span class="spacer"></span>
        <button type="button" class="btn small primary" onClick=${() => openIn(null)}>
          ${info.isPrivate ? t("{0} 시크릿 창에서 열기", name) : t("{0} 에서 열기", name)}
        </button>
      </div>
    </div>`;
  }

  return html`<div class=${classes("bar")}>
    <button type="button" class="btn plain" title=${t("뒤로")} onClick=${() => invoke("preview_back")}><${Icon} name="caret-left-bold" size=${15} /></button>
    <button type="button" class="btn plain" title=${t("앞으로")} onClick=${() => invoke("preview_forward")}><${Icon} name="caret-right-bold" size=${15} /></button>
    <button type="button" class="btn plain" title=${t("새로고침")} onClick=${() => invoke("preview_reload")}><${Icon} name="arrow-clockwise" size=${15} /></button>
    <span class="address truncate" title=${info.url}>${info.url}</span>
    ${status}
    ${boot.browsers
      .filter((browser) => browser.installed)
      .map(
        (browser) => html`<button type="button" class="btn small" key=${browser.id} title=${t("{0} 에서 열기", browser.fullName)} onClick=${() => openIn(browser.id)}>
          <${BrowserIcon} browser=${browser.id} size=${14} />${t("{0} 에서 열기", browser.name)}
        </button>`,
      )}
  </div>`;
}

render(html`<${PreviewBar} />`, document.getElementById("app"));
