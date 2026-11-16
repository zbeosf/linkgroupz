// 업데이트 창 (crates/app/src/update.rs): 새 버전 안내('나중에 · 지금 업데이트') → 내려받기 · 설치 진행 막대 → 다시 켜기.
// 설치가 끝나면 앱이 다시 켜지며 이 창도 닫힌다. 실패하면 이유와 '닫기'를 보여 준다.

import { fitWindowToContent, html, invoke, lang, render, t, useEffect, useState } from "./lib/ui.js";
import { useUpdateStatus } from "./update-ui.js";

/** 1234567 → "1.2 MB" */
function size(bytes) {
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function UpdateProgress() {
  const status = useUpdateStatus();
  useEffect(() => fitWindowToContent(), []);
  // 실패 이유는 다른 창이 메시지를 지워도(관리 창은 몇 초 뒤 지운다) 이 창을 닫을 때까지 보여 준다.
  const [failure, setFailure] = useState(null);
  useEffect(() => {
    if (status.phase !== "idle") {
      setFailure(null);
    } else if (status.message) {
      setFailure(status.message);
    }
  }, [status.phase, status.message]);

  const version = status.installingVersion ?? status.availableVersion ?? "";
  const percent = status.total ? Math.min(100, Math.floor((status.downloaded * 100) / status.total)) : null;
  let detail = t("새 버전을 확인하는 중…");
  if (status.phase === "downloading") {
    detail = percent == null ? t("내려받는 중… {0}", size(status.downloaded)) : t("내려받는 중… {0}% ({1} / {2})", percent, size(status.downloaded), size(status.total));
  } else if (status.phase === "installing") {
    detail = t("설치하는 중… 끝나면 LinkGroupz 가 다시 켜집니다");
  }
  // 설치 단계는 끝까지 찬 막대로 보여 준다. 크기를 모르면 움직이는 막대.
  const value = status.phase === "installing" ? 100 : percent;

  // 실패: 이유와 닫기 (다시 하려면 설정 → 업데이트)
  if (failure) {
    return html`<div class="update-progress">
      <div class="update-progress-head">
        <img src="app-icon.png" width="36" height="36" alt="" />
        <b>${t("업데이트하지 못했습니다")}</b>
      </div>
      <div class="caption">${failure}</div>
      <div class="row">
        <span class="caption">${t("설정 → 업데이트에서 다시 할 수 있습니다.")}</span>
        <span class="spacer"></span>
        <button type="button" class="btn primary" onClick=${() => invoke("close_window")}>${t("닫기")}</button>
      </div>
    </div>`;
  }

  // 새 버전 안내
  if (status.phase === "idle" && status.availableVersion) {
    // 바뀐 점은 한국어 다음 "### English" 줄 뒤에 영어가 있다 (xtask/src/release.rs). 화면 언어 쪽을 보여 준다.
    const [notesKo, notesEn = ""] = (status.availableNotes ?? "").split(/^### English\s*$/m);
    const notes = (lang === "en" && notesEn.trim() ? notesEn : notesKo)
      .split("\n")
      .map((line) => line.replace(/^\s*-\s*/, "").trim())
      .filter(Boolean);
    return html`<div class="update-progress">
      <div class="update-progress-head">
        <img src="app-icon.png" width="36" height="36" alt="" />
        <b>${t("LinkGroupz {0} 이 나왔습니다", status.availableVersion)}</b>
      </div>
      <div class="caption">
        ${t("지금 쓰는 버전은 {0} 입니다. 업데이트하면 LinkGroupz 를 잠깐 껐다가 새 버전으로 다시 켭니다. 그룹 · 페이지는 그대로 남습니다.", status.currentVersion)}
      </div>
      ${notes.length > 0 &&
      html`<ul class="update-notes">
        ${notes.map((note) => html`<li>${note}</li>`)}
      </ul>`}
      <div class="row">
        <span class="spacer"></span>
        <button type="button" class="btn" title=${t("이 버전은 다시 묻지 않습니다 (설정 → 업데이트에서 할 수 있습니다)")} onClick=${() => invoke("update_later")}>${t("나중에")}</button>
        <button type="button" class="btn primary" onClick=${() => invoke("update_install")}>${t("지금 업데이트")}</button>
      </div>
    </div>`;
  }

  return html`<div class="update-progress">
    <div class="update-progress-head">
      <img src="app-icon.png" width="36" height="36" alt="" />
      <b>${t("LinkGroupz {0} 로 업데이트", version)}</b>
    </div>
    <div class=${value == null ? "progress-bar indeterminate" : "progress-bar"}>
      <span style=${value == null ? "" : `width:${value}%`}></span>
    </div>
    <div class="caption">${detail}</div>
  </div>`;
}

render(html`<${UpdateProgress} />`, document.getElementById("app"));
