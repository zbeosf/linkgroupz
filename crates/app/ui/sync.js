// 그룹 동기화: 그룹을 브라우저에 같은 이름으로 열린 탭 그룹과 비교해 맞춘다 (crates/core/src/group_sync.rs).
// 탭 그룹 전체로 덮어쓰거나, 브라우저에만 있는 페이지를 골라 더하고 이 그룹에만 있는 페이지를 골라 뺀다.

import { browserInfo, Check, Empty, Favicon, html, invoke, linkName, Sheet, t, toggled, useEffect, useState } from "./lib/ui.js";

function Line({ checked, onToggle, url, title, sub }) {
  return html`<div class="line tab-line" onClick=${onToggle}>
    <${Check} state=${checked ? "on" : "off"} onToggle=${onToggle} />
    <${Favicon} url=${url} size=${16} />
    <span class="texts">
      <span class="truncate">${title}</span>
      ${sub && html`<span class="caption truncate">${sub}</span>`}
    </span>
  </div>`;
}

export function GroupSyncSheet({ group, onClose, onDone }) {
  const [plan, setPlan] = useState(null);
  const [error, setError] = useState(null);
  const [add, setAdd] = useState(new Set());
  const [remove, setRemove] = useState(new Set());
  const [useTitle, setUseTitle] = useState(true);
  const [confirmingReplace, setConfirmingReplace] = useState(false);

  const load = () => {
    setPlan(null);
    setError(null);
    invoke("sync_group_plan", { id: group.id })
      .then((loaded) => {
        setPlan(loaded);
        // 브라우저에만 있는 페이지는 더하는 쪽으로 골라 두고, 지우는 쪽은 직접 고르게 한다.
        setAdd(new Set(loaded.browserOnly));
        setRemove(new Set());
      })
      .catch((message) => setError(String(message)));
  };
  useEffect(load, [group.id]);

  const apply = async (replaceAll) => {
    try {
      await invoke("sync_group_apply", {
        id: group.id,
        choice: { tabs: plan.tabs, replaceAll, add: [...add], remove: [...remove], useTitleAsAlias: useTitle },
      });
      onDone(
        replaceAll
          ? t("‘{0}’ 을 탭 그룹과 똑같이 맞췄습니다 (페이지 {1}개)", group.name, plan.tabs.length)
          : t("‘{0}’ 에 {1}개 더하고 {2}개 뺐습니다", group.name, add.size, remove.size),
      );
      onClose();
    } catch (message) {
      setError(String(message));
    }
  };

  const browserName = plan ? browserInfo(plan.browser).name : browserInfo(group.browser).name;
  const hereOnly = plan ? group.links.filter((link) => plan.hereOnly.includes(link.id)) : [];
  const same = plan && plan.browserOnly.length === 0 && plan.hereOnly.length === 0;
  const changes = add.size + remove.size;

  return html`<${Sheet} width=${560} onClose=${onClose} onSubmit=${() => plan && changes > 0 && apply(false)} canSubmit=${Boolean(plan) && changes > 0}>
    <h2>${t("‘{0}’ 동기화", group.name)}</h2>
    <div class="secondary">
      ${plan
        ? t("{0} 의 ‘{1}’ 탭 그룹(페이지 {2}개)과 비교했습니다. 같은 페이지 {3}개.", browserName, plan.tabGroupTitle, plan.tabs.length, plan.same)
        : error
          ? ""
          : t("{0} 에 같은 이름으로 열린 탭 그룹을 찾는 중…", browserName)}
    </div>
    ${error &&
    html`<${Empty} icon="warning" title=${t("비교하지 못했습니다")} description=${error}>
      <button type="button" class="btn" onClick=${load}>${t("다시 시도")}</button>
    <//>`}
    ${same && html`<${Empty} icon="check-circle-fill" title=${t("탭 그룹과 페이지가 같습니다")} description=${t("더하거나 뺄 페이지가 없습니다.")} />`}
    ${plan &&
    !same &&
    html`
      ${plan.browserOnly.length > 0 &&
      html`<div class="caption"><b>${t("브라우저에만 있는 페이지 {0}개", plan.browserOnly.length)}</b> ${t("— 고른 것을 이 그룹 끝에 더합니다")}</div>
        <div class="list sync-list">
          ${plan.browserOnly.map((index) => {
            const tab = plan.tabs[index];
            return html`<${Line} key=${`b${index}`} checked=${add.has(index)} onToggle=${() => setAdd(toggled(add, index))} url=${tab.url} title=${tab.title || tab.url} sub=${tab.title ? tab.url : ""} />`;
          })}
        </div>`}
      ${hereOnly.length > 0 &&
      html`<div class="caption"><b>${t("이 그룹에만 있는 페이지 {0}개", hereOnly.length)}</b> ${t("— 고른 것을 그룹에서 지웁니다")}</div>
        <div class="list sync-list">
          ${hereOnly.map(
            (link) => html`<${Line}
              key=${link.id}
              checked=${remove.has(link.id)}
              onToggle=${() => setRemove(toggled(remove, link.id))}
              url=${link.url}
              title=${linkName(link)}
              sub=${link.alias ? link.url : ""}
            />`,
          )}
        </div>`}
      <label class="row" style="gap:6px">
        <input type="checkbox" checked=${useTitle} onChange=${(event) => setUseTitle(event.target.checked)} />
        <span>${t("새로 더하는 페이지는 탭 제목을 이름(alias)으로")}</span>
      </label>
    `}
    ${confirmingReplace &&
    html`<div class="callout-warning">
      ${t("그룹의 페이지를 ‘{0}’ 탭 그룹과 똑같이(순서까지) 맞춥니다.\n      {1}\n      이미 있던 페이지의 이름 · 스위치는 그대로 둡니다.", plan.tabGroupTitle, plan.hereOnly.length > 0 ? t(" 탭 그룹에 없는 페이지 {0}개는 지워집니다.", plan.hereOnly.length) : "")}
    </div>`}
    <div class="actions">
      <button type="button" class="btn" onClick=${onClose}>${same ? t("닫기") : t("취소")}</button>
      ${plan &&
      !same &&
      (confirmingReplace
        ? html`<button type="button" class="btn destructive" onClick=${() => apply(true)}>${t("덮어쓰기")}</button>`
        : html`<button type="button" class="btn" title=${t("그룹의 페이지를 탭 그룹과 똑같이 맞춥니다")} onClick=${() => setConfirmingReplace(true)}>${t("전체 덮어쓰기…")}</button>`)}
      ${plan &&
      !same &&
      html`<button type="button" class="btn primary" disabled=${changes === 0} onClick=${() => apply(false)}>
        ${t("반영 (더하기 {0} · 빼기 {1})", add.size, remove.size)}
      </button>`}
    </div>
  <//>`;
}
