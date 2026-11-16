// 브라우저에 열린 탭 · 탭 그룹 가져오기 시트

import {
  boot,
  browserInfo,
  BrowserPicker,
  Check,
  Chevron,
  colorCss,
  ColorDot,
  Empty,
  GroupIcon,
  html,
  Icon,
  invoke,
  SelectAll,
  Sheet,
  t,
  toggled,
  useEffect,
  useState,
} from "./lib/ui.js";

/** 설치된 브라우저 중에서 고른다 (그 브라우저가 없으면 기본 브라우저). */
function installedOr(browser) {
  return browserInfo(browser).installed ? browser : boot.browsers[0].id;
}

/** 브라우저가 켜져 있는지 (가져올 브라우저 고르기에 '꺼짐' 표시) */
function useBrowserStates(refreshKey) {
  const [states, setStates] = useState(null);
  useEffect(() => {
    let current = true;
    invoke("browser_states").then((loaded) => {
      if (current) {
        setStates(loaded);
      }
    });
    return () => {
      current = false;
    };
  }, [refreshKey]);
  return states;
}

function isSelectable(candidate) {
  return candidate.status === "available";
}

/** 가져오기 목록의 페이지 한 줄 */
function CandidateLine({ candidate, checked, onToggle, indent, stripe }) {
  const selectable = isSelectable(candidate);
  return html`<div class="line tab-line" style=${`padding-left:${indent}px`} onClick=${selectable ? onToggle : undefined}>
    ${stripe && html`<span class="stripe" style=${`background:${colorCss(stripe)}`}></span>`}
    <${Check} state=${checked ? "on" : "off"} disabled=${!selectable} onToggle=${onToggle} />
    <span class="texts" style=${selectable ? "" : "opacity:0.5"}>
      <span class="truncate">${candidate.tab.title || candidate.tab.url}</span>
      <span class="caption truncate">${candidate.tab.url}</span>
    </span>
    ${candidate.status === "alreadyInGroup" && html`<span class="caption">${t("이미 있음")}</span>`}
    ${candidate.status === "unsupported" && html`<span class="caption">${t("추가 불가")}</span>`}
  </div>`;
}

/** 목록 아래: 모두 펼치기 · 접기, 탭 제목을 alias 로 */
function ListOptions({ collapsedCount, collapsibleCount, onExpandAll, onCollapseAll, useTitleAsAlias, onUseTitleAsAlias }) {
  return html`<div class="row">
    <button type="button" class="btn" disabled=${collapsedCount === 0} onClick=${onExpandAll}>
      <${Icon} name="caret-double-down" size=${13} />${t("모두 펼치기")}
    </button>
    <button type="button" class="btn" disabled=${collapsibleCount === 0 || collapsedCount === collapsibleCount} onClick=${onCollapseAll}>
      <${Icon} name="caret-double-up" size=${13} />${t("모두 접기")}
    </button>
    <span class="spacer"></span>
    <label class="row" style="gap:5px">
      <input type="checkbox" checked=${useTitleAsAlias} onChange=${(event) => onUseTitleAsAlias(event.target.checked)} />
      ${t("탭 제목을 alias 로")}
    </label>
  </div>`;
}

/** 읽는 중 · 읽지 못함 · 비어 있음 */
function ListState({ loading, error, empty, loadingText, errorTitle, emptyTitle, emptyIcon, onRetry }) {
  if (loading) {
    return html`<div class="empty"><span class="spinner"></span>${loadingText}</div>`;
  }
  if (error) {
    return html`<${Empty} icon="warning" title=${errorTitle} description=${error}>
      <button type="button" class="btn" onClick=${onRetry}>${t("다시 시도")}</button>
    <//>`;
  }
  if (empty) {
    return html`<${Empty} icon=${emptyIcon} title=${emptyTitle} />`;
  }
  return null;
}

// MARK: - 열린 탭 가져오기

/**
 * 같은 창 안에서 이어지는 같은 탭 그룹의 탭을 한 덩어리로 묶어, 창 · 탭 그룹 · 페이지를 한 줄 목록으로 편다
 * (브라우저 탭 그룹은 탭 바에서 붙어 있다).
 */
function tabRows(candidates, collapsed) {
  const rows = [];
  const windows = [...new Set(candidates.map((candidate) => candidate.tab.windowIndex))].sort((a, b) => a - b);
  for (const windowIndex of windows) {
    const members = candidates.filter((candidate) => candidate.tab.windowIndex === windowIndex);
    const windowID = `window-${windowIndex}`;
    rows.push({ kind: "window", id: windowID, windowIndex, members });
    if (collapsed.has(windowID)) {
      continue;
    }
    let block = null;
    for (const candidate of members) {
      const info = candidate.tab.tabGroup;
      if (!info) {
        block = null;
        rows.push({ kind: "page", id: `page-${candidate.id}`, candidate });
        continue;
      }
      if (!block || block.info.id !== info.id) {
        block = { kind: "tabGroup", id: `group-${info.id}-${candidate.id}`, info, members: [] };
        rows.push(block);
      }
      block.members.push(candidate);
      if (!collapsed.has(block.id)) {
        rows.push({ kind: "page", id: `page-${candidate.id}`, candidate, groupColor: info.color });
      }
    }
  }
  return rows;
}

/** 고른 브라우저(Chrome · Edge · Whale)에 열린 탭을 골라 그룹에 한꺼번에 추가한다. 탭 그룹에 속한 탭은 그룹째 묶어 보여준다. */
export function ImportTabsSheet({ group, onClose }) {
  // 그룹을 여는 브라우저에서 가져오는 경우가 많아 그 브라우저로 시작한다.
  const [browser, setBrowser] = useState(installedOr(group.browser));
  const [candidates, setCandidates] = useState([]);
  const [note, setNote] = useState(null);
  const [selected, setSelected] = useState(new Set());
  const [useTitleAsAlias, setUseTitleAsAlias] = useState(true);
  const [error, setError] = useState(null);
  const [loading, setLoading] = useState(true);
  const [collapsed, setCollapsed] = useState(new Set());
  const [reload, setReload] = useState(0);
  const states = useBrowserStates(`${browser}-${reload}`);

  useEffect(() => {
    let current = true;
    setLoading(true);
    setError(null);
    invoke("load_tabs", { browser, groupId: group.id })
      .then((loaded) => {
        if (!current) {
          return;
        }
        setCandidates(loaded.candidates);
        setNote(loaded.note);
        // 사용법 스크린샷용: 첫 탭 그룹을 골라 둔다
        const first = boot.scene === "importTabs" && loaded.candidates.find((candidate) => candidate.tab.tabGroup)?.tab.tabGroup;
        setSelected(
          first
            ? new Set(loaded.candidates.filter((c) => isSelectable(c) && c.tab.tabGroup?.id === first.id).map((c) => c.id))
            : new Set(),
        );
      })
      .catch((message) => {
        if (!current) {
          return;
        }
        setCandidates([]);
        setSelected(new Set());
        setNote(null);
        setError(String(message));
      })
      .finally(() => {
        if (current) {
          setLoading(false);
        }
      });
    return () => {
      current = false;
    };
  }, [browser, reload]);

  const selectable = candidates.filter(isSelectable).map((candidate) => candidate.id);
  const rows = tabRows(candidates, collapsed);
  const collapsible = tabRows(candidates, new Set()).filter((row) => row.kind !== "page").map((row) => row.id);
  const ready = !loading && !error && candidates.length > 0;

  const selectMembers = (members) => setSelected(new Set([...selected, ...members.filter(isSelectable).map((c) => c.id)]));
  const deselectMembers = (members) => {
    const ids = new Set(members.map((candidate) => candidate.id));
    setSelected(new Set([...selected].filter((id) => !ids.has(id))));
  };
  const header = (row, label, indent) => html`<div class="line tab-line" key=${row.id} style=${`padding-left:${indent}px`}>
    <${Chevron} expanded=${!collapsed.has(row.id)} onToggle=${() => setCollapsed(toggled(collapsed, row.id))} />
    <${SelectAll}
      selected=${row.members.filter((candidate) => selected.has(candidate.id)).length}
      selectable=${row.members.filter(isSelectable).length}
      onSelectAll=${() => selectMembers(row.members)}
      onDeselectAll=${() => deselectMembers(row.members)}
    >${label}<//>
    <span class="fill" onClick=${() => setCollapsed(toggled(collapsed, row.id))}></span>
  </div>`;

  const add = async () => {
    const links = candidates
      .filter((candidate) => selected.has(candidate.id) && candidate.url)
      .map((candidate) => ({ url: candidate.url, alias: useTitleAsAlias ? candidate.tab.title : null }));
    await invoke("add_links", { groupId: group.id, links });
    onClose();
  };

  return html`<${Sheet} width=${640} onClose=${onClose} onSubmit=${add} canSubmit=${selected.size > 0}>
    <div class="row">
      <h2>${t("열린 탭 가져오기")}</h2>
      <${Icon} name="arrow-right" size=${14} class="secondary" />
      <${GroupIcon} icon=${group.icon} color=${group.color} size=${20} />
      <h2 class="truncate">${group.name}</h2>
      <span class="spacer"></span>
      <button type="button" class="btn plain" title=${t("탭 목록 다시 읽기")} disabled=${loading} onClick=${() => setReload(reload + 1)}>
        <${Icon} name="arrow-clockwise" size=${15} />
      </button>
    </div>
    <div class="row">
      <span class="secondary">${t("가져올 브라우저")}</span>
      <${BrowserPicker} value=${browser} onChange=${setBrowser} states=${states} />
    </div>
    ${note && html`<div class="row caption"><${Icon} name="info" size=${14} />${note}</div>`}
    ${ready &&
    html`<div class="row" style="padding-left:8px;margin-bottom:-6px">
      <${SelectAll}
        selected=${selected.size}
        selectable=${selectable.length}
        onSelectAll=${() => setSelected(new Set(selectable))}
        onDeselectAll=${() => setSelected(new Set())}
      ><b style="font-size:15px">${t("전체 ({0}/{1})", selected.size, selectable.length)}</b><//>
      ${selectable.length === 0 && html`<span class="secondary">${t("가져올 수 있는 탭이 없습니다 (모두 이미 있거나 추가할 수 없는 주소)")}</span>`}
    </div>`}
    <div class="list tab-list">
      ${ready
        ? rows.map((row) => {
            if (row.kind === "window") {
              return header(
                row,
                html`<b>${row.windowIndex === 1 ? t("창 1 (맨 앞)") : t("창 {0}", row.windowIndex)}</b><span class="caption">${t("탭 {0}개", row.members.length)}</span>`,
                4,
              );
            }
            if (row.kind === "tabGroup") {
              const chosen = row.members.filter((candidate) => selected.has(candidate.id)).length;
              return header(
                row,
                html`<span class="pill truncate" style=${`background:${colorCss(row.info.color)}`}>${row.info.title || t("이름 없는 그룹")}</span>
                  <span class="caption">${t("{0}/{1} 선택 · 탭 {2}개", chosen, row.members.filter(isSelectable).length, row.members.length)}</span>`,
                24,
              );
            }
            return html`<${CandidateLine}
              key=${row.id}
              candidate=${row.candidate}
              checked=${selected.has(row.candidate.id)}
              onToggle=${() => setSelected(toggled(selected, row.candidate.id))}
              indent=${row.groupColor ? 48 : 24}
              stripe=${row.groupColor}
            />`;
          })
        : html`<${ListState}
            loading=${loading}
            error=${error}
            empty=${true}
            loadingText=${t("{0} 탭을 읽는 중…", browserInfo(browser).name)}
            errorTitle=${t("탭을 읽지 못했습니다")}
            emptyTitle=${t("열린 탭이 없습니다")}
            emptyIcon="browsers"
            onRetry=${() => setReload(reload + 1)}
          />`}
    </div>
    <${ListOptions}
      collapsedCount=${collapsed.size}
      collapsibleCount=${collapsible.length}
      onExpandAll=${() => setCollapsed(new Set())}
      onCollapseAll=${() => setCollapsed(new Set(collapsible))}
      useTitleAsAlias=${useTitleAsAlias}
      onUseTitleAsAlias=${setUseTitleAsAlias}
    />
    <div class="actions">
      <button type="button" class="btn" onClick=${onClose}>${t("취소")}</button>
      <button type="button" class="btn primary" disabled=${selected.size === 0} onClick=${add}>${t("추가 ({0})", selected.size)}</button>
    </div>
  <//>`;
}

// MARK: - 탭 그룹 가져오기

/**
 * 고른 브라우저(Chrome · Edge · Whale)의 탭 그룹을 골라 같은 이름 · 색의 그룹으로 만든다. 그 브라우저에 확장이 있어야 한다.
 * 새로 만든 그룹은 가져온 브라우저에서 열리도록 설정한다.
 *
 * 탭 그룹 아래에 그룹의 페이지를 펼쳐 보여주고, 페이지를 하나씩 고르거나 그룹 체크박스로 한 번에 고른다.
 * 고른 페이지가 하나 이상인 탭 그룹만 가져온다. 같은 이름의 그룹이 이미 있으면 그 그룹에 페이지를 더하고,
 * 그 그룹에 이미 있는 페이지는 '이미 있음'으로 고를 수 없다.
 */
export function ImportTabGroupsSheet({ onClose, onImported }) {
  const [browser, setBrowser] = useState(boot.browsers[0].id);
  const [tabGroups, setTabGroups] = useState([]);
  /** 탭 그룹 id → 고른 후보 id 들 */
  const [selected, setSelected] = useState(new Map());
  const [collapsed, setCollapsed] = useState(new Set());
  const [useTitleAsAlias, setUseTitleAsAlias] = useState(true);
  const [error, setError] = useState(null);
  const [loading, setLoading] = useState(true);
  const [reload, setReload] = useState(0);
  const [plan, setPlan] = useState({ newGroups: 0, additions: 0, pageCount: 0 });
  // 브라우저에 저장된 닫힌 탭 그룹 (켜면 열린 그룹 아래에 접힌 채로 나온다)
  const [showClosed, setShowClosed] = useState(boot.scene === "importClosedGroups");
  const [closedGroups, setClosedGroups] = useState([]);
  const [closedLoading, setClosedLoading] = useState(false);
  const states = useBrowserStates(`${browser}-${reload}`);

  const idOf = (loaded) => loaded.tabGroup.info.id;
  const selectableOf = (loaded) => loaded.candidates.filter(isSelectable).map((candidate) => candidate.id);
  const everything = (groups) => new Map(groups.map((loaded) => [idOf(loaded), new Set(selectableOf(loaded))]));

  useEffect(() => {
    let current = true;
    setLoading(true);
    setError(null);
    invoke("load_tab_groups", { browser })
      .then((loaded) => {
        if (!current) {
          return;
        }
        setTabGroups(loaded);
        // 사용법 스크린샷용: 전체를 골라 둔다
        setSelected(boot.scene === "importGroups" ? everything(loaded) : new Map());
      })
      .catch((message) => {
        if (!current) {
          return;
        }
        setTabGroups([]);
        setSelected(new Map());
        setError(String(message));
      })
      .finally(() => {
        if (current) {
          setLoading(false);
        }
      });
    return () => {
      current = false;
    };
  }, [browser, reload]);

  // 닫힌 탭 그룹: 열린 그룹을 다 읽은 뒤(브라우저가 꺼져 있어 못 읽었어도) 읽는다. 이름 · 색이 같은 열린 그룹은 빠진다.
  useEffect(() => {
    if (!showClosed || loading) {
      setClosedGroups([]);
      return undefined;
    }
    let current = true;
    setClosedLoading(true);
    invoke("load_closed_tab_groups", { browser, open: error ? [] : tabGroups.map((loaded) => loaded.tabGroup) })
      .then((loaded) => {
        if (current) {
          setClosedGroups(loaded);
          setCollapsed((previous) => new Set([...previous, ...loaded.map(idOf)]));
        }
      })
      .catch(() => current && setClosedGroups([]))
      .finally(() => current && setClosedLoading(false));
    return () => {
      current = false;
    };
  }, [showClosed, loading, tabGroups, error]);

  const openGroups = !loading && !error ? tabGroups : [];
  const allGroups = [...openGroups, ...closedGroups];

  const selection = () => ({
    tabGroups: allGroups.map((loaded) => loaded.tabGroup),
    selections: Object.fromEntries([...selected].map(([id, ids]) => [id, [...ids]])),
    useTitleAsAlias,
    browser,
  });

  // 고른 대로 가져오면 무엇이 생기는지는 앱이 정한다 (같은 이름 묶기 · 중복 제거).
  useEffect(() => {
    let current = true;
    invoke("plan_tab_group_import", { selection: selection() }).then((summary) => {
      if (current) {
        setPlan(summary);
      }
    });
    return () => {
      current = false;
    };
  }, [tabGroups, closedGroups, selected, useTitleAsAlias, browser]);

  const selectableTotal = allGroups.reduce((sum, loaded) => sum + selectableOf(loaded).length, 0);
  const chosenTotal = [...selected.values()].reduce((sum, ids) => sum + ids.size, 0);
  const ready = !loading && allGroups.length > 0;
  const setGroup = (id, ids) => setSelected(new Map(selected).set(id, ids));

  const parts = [];
  if (plan.newGroups > 0) {
    parts.push(t("새 그룹 {0}개", plan.newGroups));
  }
  if (plan.additions > 0) {
    parts.push(t("기존 그룹 {0}개에 추가", plan.additions));
  }
  parts.push(t("페이지 {0}개", plan.pageCount));

  const run = async () => {
    const first = await invoke("import_tab_groups", { selection: selection() });
    onImported(first);
    onClose();
  };

  const name = browserInfo(browser).name;
  return html`<${Sheet} width=${640} onClose=${onClose} onSubmit=${run} canSubmit=${chosenTotal > 0}>
    <div class="row">
      <h2>${t("탭 그룹 가져오기")}</h2>
      <span class="spacer"></span>
      <button type="button" class="btn plain" title=${t("탭 그룹 다시 읽기")} disabled=${loading} onClick=${() => setReload(reload + 1)}>
        <${Icon} name="arrow-clockwise" size=${15} />
      </button>
    </div>
    <div class="row">
      <span class="secondary">${t("가져올 브라우저")}</span>
      <${BrowserPicker} value=${browser} onChange=${setBrowser} states=${states} />
    </div>
    <div class="caption">
      ${t("고른 탭 그룹마다 같은 이름 · 색의 그룹을 새로 만들고 고른 페이지를 넣습니다. 새 그룹은 {0} 에서 열립니다. 같은 이름의 그룹이 이미 있으면 그 그룹에 더합니다.", name)}
    </div>
    <div class="row caption" style="color:var(--warn);align-items:flex-start">
      <${Icon} name="warning" size=${14} />
      <span>${t("탭 그룹에 속하지 않은 탭은 여기 나오지 않습니다. 그런 탭은 그룹을 먼저 만든 뒤, 그룹 화면의 ‘열린 탭 가져오기’로 추가해 주세요.")}</span>
    </div>
    <div class="row">
      <${Check} state=${showClosed ? "on" : "off"} onToggle=${() => setShowClosed(!showClosed)}>${t("닫힌 탭 그룹 보기")}<//>
      <span class="caption">
        ${closedLoading
          ? t("읽는 중…")
          : showClosed
            ? t("{0} 에 저장된 닫은 탭 그룹 {1}개{2}", name, closedGroups.length, error ? t(" (브라우저가 꺼져 있어 저장된 그룹을 모두 닫힌 그룹으로 보여 줍니다)") : "")
            : t("{0} 에 저장해 둔(닫아 둔) 탭 그룹도 가져옵니다", name)}
      </span>
    </div>
    ${ready &&
    html`<div class="row" style="padding-left:8px;margin-bottom:-6px">
      <${SelectAll}
        selected=${chosenTotal}
        selectable=${selectableTotal}
        onSelectAll=${() => setSelected(everything(allGroups))}
        onDeselectAll=${() => setSelected(new Map())}
      ><b style="font-size:15px">${t("전체 그룹 ({0}/{1})", chosenTotal, selectableTotal)}</b><//>
      ${selectableTotal === 0 && html`<span class="secondary">${t("모든 페이지가 이미 그룹에 있습니다")}</span>`}
    </div>`}
    <div class="list tab-list" style="max-height:480px">
      ${ready
        ? allGroups.map((loaded, index) => {
            const id = idOf(loaded);
            const closed = loaded.tabGroup.info.isClosed;
            const firstClosed = closed && (index === 0 || !allGroups[index - 1].tabGroup.info.isClosed);
            const chosen = selected.get(id) ?? new Set();
            const expanded = !collapsed.has(id);
            const color = loaded.tabGroup.info.color;
            return html`
              ${firstClosed && html`<div class="line closed-header" key="closed-header"><${Icon} name="archive" size=${13} />${t("닫힌 탭 그룹")}</div>`}
              <div class="line tab-line" key=${`group-${id}`}>
                <${Chevron} expanded=${expanded} onToggle=${() => setCollapsed(toggled(collapsed, id))} />
                <${SelectAll}
                  selected=${chosen.size}
                  selectable=${selectableOf(loaded).length}
                  onSelectAll=${() => setGroup(id, new Set(selectableOf(loaded)))}
                  onDeselectAll=${() => setGroup(id, new Set())}
                >
                  <span class="pill truncate" style=${`background:${colorCss(color)}`}>${loaded.displayTitle}</span>
                  <span class="caption">${t("{0}/{1} 선택 · {2}", chosen.size, selectableOf(loaded).length, closed ? t("닫힌 그룹") : t("창 {0}", loaded.tabGroup.windowIndex))}</span>
                <//>
                <span class="fill" onClick=${() => setCollapsed(toggled(collapsed, id))}></span>
                ${loaded.existing &&
                html`<span class="row caption" style="gap:4px">
                  <${Icon} name="arrow-bend-down-right" size=${13} />
                  <${ColorDot} color=${loaded.existing.color} size=${8} />
                  ${t("기존 ‘{0}’ 에 추가", loaded.existing.name)}
                </span>`}
              </div>
              ${expanded &&
              loaded.candidates.map(
                (candidate) => html`<${CandidateLine}
                  key=${`group-${id}-page-${candidate.id}`}
                  candidate=${candidate}
                  checked=${chosen.has(candidate.id)}
                  onToggle=${() => setGroup(id, toggled(chosen, candidate.id))}
                  indent=${28}
                  stripe=${color}
                />`,
              )}
            `;
          })
        : html`<${ListState}
            loading=${loading}
            error=${error}
            empty=${true}
            loadingText=${t("{0} 탭 그룹을 읽는 중…", name)}
            errorTitle=${t("탭 그룹을 읽지 못했습니다")}
            emptyTitle=${t("{0} 에 탭 그룹이 없습니다", name)}
            emptyIcon="stack"
            onRetry=${() => setReload(reload + 1)}
          />`}
    </div>
    <${ListOptions}
      collapsedCount=${collapsed.size}
      collapsibleCount=${allGroups.length}
      onExpandAll=${() => setCollapsed(new Set())}
      onCollapseAll=${() => setCollapsed(new Set(allGroups.map(idOf)))}
      useTitleAsAlias=${useTitleAsAlias}
      onUseTitleAsAlias=${setUseTitleAsAlias}
    />
    <div class="actions">
      <button type="button" class="btn" onClick=${onClose}>${t("취소")}</button>
      <button type="button" class="btn primary" disabled=${chosenTotal === 0} onClick=${run}>${t("가져오기 ({0})", parts.join(" · "))}</button>
    </div>
  <//>`;
}
