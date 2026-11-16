// 관리 창 오른쪽에 보이는 보관함. 보관 그룹 · 보관 페이지를 접힌 채로 보여주고, 펼쳐서 복원 · 영구 삭제한다.
//
// 끌어 놓기: 보관 그룹은 왼쪽 그룹 목록의 그 자리로 복원 · 보관 그룹끼리 순서 바꾸기,
// 보관 페이지 · 보관 그룹 안 페이지는 왼쪽 그룹 위에 놓아 복원, 보관 페이지는 보관 그룹 안으로 넣거나 순서 바꾸기.
// 더블클릭하면 보관한 그룹 · 페이지를 바로 연다.

import {
  boot,
  BrowserBadge,
  Chevron,
  classes,
  colorCss,
  ColorDot,
  Confirm,
  Empty,
  Favicon,
  formatMonthDay,
  fromTimestamp,
  GroupIcon,
  html,
  Icon,
  invoke,
  linkName,
  MenuButton,
  SearchField,
  SearchScopeButton,
  showMenu,
  t,
  toggled,
  UNDO_KEY,
  useFiltered,
  useState,
} from "./lib/ui.js";
import { GroupEditSheet } from "./sheets.js";
import { archiveDrag, setArchiveDrag } from "./lib/drag.js";

/** 마우스가 줄의 위쪽 절반이면 그 줄 앞, 아래쪽이면 뒤 */
function insertionIndex(event, index) {
  const box = event.currentTarget.getBoundingClientRect();
  return event.clientY < box.top + box.height / 2 ? index : index + 1;
}

/** '보관 그룹 (3)' 처럼 개수가 붙은 접기 · 펼치기 줄 */
function SectionTitle({ title, count, expanded, disabled, onToggle }) {
  return html`<div class=${classes("section-title", !disabled && "clickable")} onClick=${disabled ? undefined : onToggle}>
    <${Chevron} expanded=${expanded} disabled=${disabled} onToggle=${onToggle} />
    <span>${title}</span>
    <span class="count">${count}</span>
    <span class="fill"></span>
  </div>`;
}

/** 보관함 안의 페이지 한 줄 (사이트 아이콘 · 이름 · 주소) */
function ArchivedPage({ link }) {
  return html`<div class="archived-page">
    <${Favicon} url=${link.url} size=${16} />
    <span class="texts">
      <span class="truncate">${linkName(link)}</span>
      <span class="caption truncate">${link.url}</span>
    </span>
  </div>`;
}

export function ArchiveView({ store, onShowGroup }) {
  const scene = boot.scene;
  const [groupsExpanded, setGroupsExpanded] = useState(scene === "archive");
  const [linksExpanded, setLinksExpanded] = useState(scene === "archive");
  const [expandedGroups, setExpandedGroups] = useState(
    () => new Set(scene === "archive" ? store.archive.groups.slice(0, 1).map((item) => item.group.id) : []),
  );
  const [newGroupRestore, setNewGroupRestore] = useState(null);
  const [pendingDelete, setPendingDelete] = useState(null);
  const [notice, setNotice] = useState(null);
  const [query, setQuery] = useState(scene === "archiveSearch" ? "figma" : "");
  // 끌어서 놓을 자리 { key, position: "before" | "after" | "onto" }
  const [drop, setDrop] = useState(null);
  const endDrag = () => {
    setArchiveDrag(null);
    setDrop(null);
  };
  const mark = (key) => {
    const position = drop?.key === key ? drop.position : null;
    return position === "before" ? "insert-before" : position === "after" ? "insert-after" : position === "onto" ? "drop-target" : null;
  };

  const searching = query.trim() !== "";
  // 검색어에 맞는 것만 남긴 보관함 (검색어가 없으면 전체)
  const shown = useFiltered("filter_archive", query, store, store.archive);
  const archiveEmpty = store.archive.groups.length === 0 && store.archive.links.length === 0;
  const shownEmpty = shown.groups.length === 0 && shown.links.length === 0;

  const restoreGroup = async (group) => {
    await invoke("restore_groups", { ids: [group.id] });
    setNotice({ text: t("‘{0}’ 그룹을 그룹 목록 끝에 복원했습니다.", group.name), groupID: group.id });
  };
  const restoreLink = async (item, group) => {
    const result = await invoke("restore_links", { ids: [item.link.id], targetId: group.id });
    setNotice({
      text: result.duplicates > 0 ? t("‘{0}’ 에 같은 주소가 이미 있어 보관함에서만 뺐습니다.", group.name) : t("‘{0}’ 에 복원했습니다.", group.name),
      groupID: group.id,
    });
  };
  /** 원래 그룹(있으면 맨 위) · 다른 그룹 · 새 그룹 */
  const restoreItems = (item) => {
    const original = store.groups.find((group) => group.id === item.groupID);
    const others = store.groups.filter((group) => group.id !== original?.id);
    return [
      original && { label: t("원래 그룹 ‘{0}’", original.name), action: () => restoreLink(item, original) },
      original && "-",
      ...others.map((group) => ({ label: group.name, action: () => restoreLink(item, group) })),
      others.length > 0 && "-",
      {
        label: t("새 그룹 만들기…"),
        action: () => setNewGroupRestore({ template: { name: item.groupName, color: item.groupColor }, linkIDs: [item.link.id] }),
      },
    ];
  };

  /** 보관 그룹 안 페이지를 복원할 그룹 */
  const restoreGroupPageItems = (group, link) =>
    store.groups.length === 0
      ? [{ label: t("복원할 그룹이 없습니다"), disabled: true }]
      : store.groups.map((target) => ({
          label: target.name,
          action: async () => {
            const result = await invoke("restore_archived_group_links", { groupId: group.id, ids: [link.id], targetId: target.id });
            setNotice({
              text: result.duplicates > 0 ? t("‘{0}’ 에 같은 주소가 이미 있어 보관 그룹에서만 뺐습니다.", target.name) : t("‘{0}’ 에 복원했습니다.", target.name),
              groupID: target.id,
            });
          },
        }));

  /** 보관 그룹 안 페이지 한 줄: 끌어서 순서 · 왼쪽 그룹에 놓아 복원, 더블클릭으로 열기 */
  const groupPageRow = (group, link, index) => {
    const key = `gp:${link.id}`;
    const fullLinks = store.archive.groups.find((other) => other.group.id === group.id)?.group.links ?? group.links;
    const order = fullLinks.findIndex((item) => item.id === link.id);
    const here = () => !searching && archiveDrag?.kind === "archivedGroupLinks" && archiveDrag.groupId === group.id;
    return html`<div
      key=${link.id}
      class=${classes("archived-page-row", mark(key))}
      draggable=${true}
      title=${t("더블클릭: 열기 · 끌어서 왼쪽 그룹에 놓으면 복원")}
      onDragStart=${(event) => {
        setArchiveDrag({ kind: "archivedGroupLinks", groupId: group.id, ids: [link.id] });
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", link.url);
      }}
      onDragEnd=${endDrag}
      onDragOver=${(event) => {
        if (here()) {
          event.preventDefault();
          setDrop({ key, position: insertionIndex(event, order) === order ? "before" : "after" });
        }
      }}
      onDrop=${(event) => {
        event.preventDefault();
        if (here()) {
          const from = archiveDrag.ids.map((id) => fullLinks.findIndex((item) => item.id === id)).filter((at) => at >= 0);
          invoke("move_links_in_archived_group", { groupId: group.id, from, to: insertionIndex(event, order) });
        }
        endDrag();
      }}
      onDblClick=${() => invoke("open_archived_links", { ids: [link.id], groupId: group.id })}
    >
      <${ArchivedPage} link=${link} />
      <span class="spacer"></span>
      <span class="row-buttons" onDblClick=${(event) => event.stopPropagation()}>
        <${MenuButton} class="btn small" title=${t("이 페이지만 복원할 그룹 고르기")} items=${() => restoreGroupPageItems(group, link)}>
          <${Icon} name="arrow-u-up-left" size=${13} />${t("복원")}<${Icon} name="caret-down-bold" size=${9} />
        <//>
        <button
          type="button"
          class="btn plain danger-text"
          title=${t("이 페이지 영구 삭제")}
          onClick=${() => setPendingDelete({ kind: "groupLink", groupID: group.id, id: link.id })}
        >
          <${Icon} name="trash" size=${15} />
        </button>
      </span>
    </div>`;
  };

  const groupRow = (item, index) => {
    const group = item.group;
    const key = `g:${group.id}`;
    const position = (event) => {
      if (archiveDrag?.kind === "archivedLinks") {
        return "onto"; // 보관 페이지를 이 보관 그룹 안으로
      }
      return insertionIndex(event, index) === index ? "before" : "after";
    };
    const droppable = () =>
      !searching && ((archiveDrag?.kind === "archivedGroups" && !archiveDrag.ids.includes(group.id)) || archiveDrag?.kind === "archivedLinks");
    const total = store.archive.groups.find((other) => other.group.id === group.id)?.group.links.length ?? group.links.length;
    // 그룹 안 페이지로 찾은 결과면 맞는 페이지가 보이게 펼친다.
    const matchedByPages = searching && group.links.length < total;
    const expanded = expandedGroups.has(group.id) || matchedByPages;
    const when = t("{0} 보관", formatMonthDay(fromTimestamp(item.archivedAt)));
    const menu = (event) =>
      showMenu(event, [
        { label: t("복원"), action: () => restoreGroup(group) },
        "-",
        { label: t("영구 삭제"), destructive: true, action: () => setPendingDelete({ kind: "group", id: group.id }) },
      ]);
    return html`<div
      class=${classes("archived-group", mark(key))}
      key=${group.id}
      onContextMenu=${menu}
      onDragOver=${(event) => {
        if (droppable()) {
          event.preventDefault();
          setDrop({ key, position: position(event) });
        }
      }}
      onDrop=${(event) => {
        event.preventDefault();
        if (droppable()) {
          if (archiveDrag.kind === "archivedLinks") {
            invoke("move_archived_links_into_group", { ids: archiveDrag.ids, groupId: group.id });
            setNotice({ text: t("보관 페이지를 보관 그룹 ‘{0}’ 에 넣었습니다.", group.name) });
          } else {
            const order = store.archive.groups.map((other) => other.group.id);
            const from = archiveDrag.ids.map((id) => order.indexOf(id)).filter((at) => at >= 0);
            const full = order.indexOf(group.id);
            invoke("move_archived_groups", { from, to: position(event) === "before" ? full : full + 1 });
          }
        }
        endDrag();
      }}
    >
      <div
        class="group-card"
        style=${`--tint:${colorCss(group.color)}`}
        draggable=${true}
        title=${t("더블클릭: 그룹 열기 · 끌어서 왼쪽 그룹 목록에 놓으면 그 자리에 복원")}
        onDragStart=${(event) => {
          setArchiveDrag({ kind: "archivedGroups", ids: [group.id] });
          event.dataTransfer.effectAllowed = "move";
          event.dataTransfer.setData("text/plain", group.name);
        }}
        onDragEnd=${endDrag}
        onDblClick=${() => invoke("open_archived_group", { id: group.id })}
      >
        <${Chevron}
          expanded=${expanded}
          disabled=${group.links.length === 0 || matchedByPages}
          onToggle=${() => setExpandedGroups(toggled(expandedGroups, group.id))}
        />
        <${GroupIcon} icon=${group.icon} color=${group.color} size=${26} />
        <span class="names">
          <span class="name truncate">${group.name}</span>
          <span class="tags truncate">
            ${matchedByPages ? t("페이지 {0}개 중 {1}개 맞음 · {2}", total, group.links.length, when) : t("페이지 {0}개 · {1}", total, when)}
          </span>
        </span>
        <${BrowserBadge} browser=${group.browser} />
        <button type="button" class="btn small" title=${t("그룹 목록 끝으로 되돌리기")} onClick=${() => restoreGroup(group)}>
          <${Icon} name="arrow-u-up-left" size=${13} />${t("복원")}
        </button>
        <button type="button" class="btn plain danger-text" title=${t("영구 삭제")} onClick=${() => setPendingDelete({ kind: "group", id: group.id })}>
          <${Icon} name="trash" size=${15} />
        </button>
      </div>
      ${expanded && group.links.map((link, linkIndex) => groupPageRow(group, link, linkIndex))}
    </div>`;
  };

  const linkRow = (item, index) => {
    const key = `l:${item.link.id}`;
    const here = () => !searching && archiveDrag?.kind === "archivedLinks" && !archiveDrag.ids.includes(item.link.id);
    const order = store.archive.links.findIndex((other) => other.link.id === item.link.id);
    const menu = (event) =>
      showMenu(event, [
        { label: t("복원"), submenu: restoreItems(item) },
        "-",
        { label: t("영구 삭제"), destructive: true, action: () => setPendingDelete({ kind: "link", id: item.link.id }) },
      ]);
    return html`<div
      class=${classes("row archived-link-row", mark(key))}
      key=${item.link.id}
      style="padding:1px 0"
      draggable=${true}
      title=${t("더블클릭: 열기 · 끌어서 왼쪽 그룹에 놓으면 복원 · 보관 그룹에 놓으면 그 안으로")}
      onContextMenu=${menu}
      onDragStart=${(event) => {
        setArchiveDrag({ kind: "archivedLinks", ids: [item.link.id] });
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", item.link.url);
      }}
      onDragEnd=${endDrag}
      onDragOver=${(event) => {
        if (here()) {
          event.preventDefault();
          setDrop({ key, position: insertionIndex(event, order) === order ? "before" : "after" });
        }
      }}
      onDrop=${(event) => {
        event.preventDefault();
        if (here()) {
          const ids = store.archive.links.map((other) => other.link.id);
          invoke("move_archived_links", { from: archiveDrag.ids.map((id) => ids.indexOf(id)).filter((at) => at >= 0), to: insertionIndex(event, order) });
        }
        endDrag();
      }}
      onDblClick=${() => invoke("open_archived_links", { ids: [item.link.id] })}
    >
      <${ArchivedPage} link=${item.link} />
      <span class="spacer"></span>
      <span class="row caption" style="gap:4px;flex:none" title=${t("‘{0}’ 에서 보관", item.groupName)}>
        <${ColorDot} color=${item.groupColor} size=${8} />
        ${item.groupName} · ${formatMonthDay(fromTimestamp(item.archivedAt))}
      </span>
      <span class="row" style="gap:4px;flex:none" onDblClick=${(event) => event.stopPropagation()}>
        <${MenuButton} class="btn small" title=${t("복원할 그룹 고르기")} items=${() => restoreItems(item)}>
          <${Icon} name="arrow-u-up-left" size=${13} />${t("복원")}<${Icon} name="caret-down-bold" size=${9} />
        <//>
        <button type="button" class="btn plain danger-text" title=${t("영구 삭제")} onClick=${() => setPendingDelete({ kind: "link", id: item.link.id })}>
          <${Icon} name="trash" size=${15} />
        </button>
      </span>
    </div>`;
  };

  const deleteNow = async () => {
    if (pendingDelete.kind === "group") {
      await invoke("delete_archived_groups", { ids: [pendingDelete.id] });
    } else if (pendingDelete.kind === "groupLink") {
      await invoke("delete_archived_group_links", { groupId: pendingDelete.groupID, ids: [pendingDelete.id] });
    } else {
      await invoke("delete_archived_links", { ids: [pendingDelete.id] });
    }
    setPendingDelete(null);
  };

  return html`<div class="archive">
    <div class="head">
      <span class="box"><${Icon} name="archive-fill" size=${19} /></span>
      <div class="column" style="gap:2px;min-width:0">
        <h1>${t("보관함")}</h1>
        <span class="caption">${t("보관한 그룹 · 페이지는 메뉴바 · 그룹 목록 · 검색에 나오지 않습니다.")}</span>
      </div>
      <span class="spacer"></span>
      <div class="column" style="gap:6px;align-items:flex-end;flex:none">
        <div class="row" style="width:310px;gap:4px"><${SearchField} value=${query} onChange=${setQuery} placeholder=${t("보관함에서 찾기 (이름 · 태그 · URL)")} /><${SearchScopeButton} /></div>
        ${notice &&
        html`<div class="row caption" style="max-width:360px;justify-content:flex-end">
          <span>${notice.text}</span>
          ${notice.groupID && store.groups.some((group) => group.id === notice.groupID) &&
          html`<button type="button" class="btn small" onClick=${() => onShowGroup(notice.groupID)}>${t("보기")}</button>`}
        </div>`}
      </div>
    </div>
    <div class="divider"></div>
    ${archiveEmpty
      ? html`<${Empty}
          icon="archive"
          title=${t("보관함이 비어 있습니다")}
          description=${t("그룹이나 페이지를 오른쪽 클릭해 ‘보관함으로 이동’을 고르면 여기에 모입니다.")}
        />`
      : searching && shownEmpty
        ? html`<${Empty} icon="magnifying-glass" title=${t("‘{0}’ 에 맞는 결과가 없습니다", query.trim())} description=${t("다른 검색어로 찾아보세요.")} />`
        : html`<div class="body">
            ${/* 검색 중에는 결과가 있는 칸만 펼쳐서 보여준다. */ ""}
            ${(!searching || shown.groups.length > 0) &&
            html`<div>
              <${SectionTitle}
                title=${t("보관 그룹")}
                count=${shown.groups.length}
                expanded=${groupsExpanded || searching}
                disabled=${searching}
                onToggle=${() => setGroupsExpanded(!groupsExpanded)}
              />
              ${(groupsExpanded || searching) && shown.groups.map(groupRow)}
            </div>`}
            ${(!searching || shown.links.length > 0) &&
            html`<div style="margin-top:16px">
              <${SectionTitle}
                title=${t("보관 페이지")}
                count=${shown.links.length}
                expanded=${linksExpanded || searching}
                disabled=${searching}
                onToggle=${() => setLinksExpanded(!linksExpanded)}
              />
              ${(linksExpanded || searching) && shown.links.map(linkRow)}
            </div>`}
          </div>`}
    ${newGroupRestore &&
    html`<${GroupEditSheet}
      original=${newGroupRestore.template}
      knownTags=${store.allTags}
      title=${t("새 그룹으로 복원")}
      saveTitle=${t("복원")}
      onClose=${() => setNewGroupRestore(null)}
      onSave=${async (input) => {
        const restored = await invoke("restore_links_to_new_group", { ids: newGroupRestore.linkIDs, input });
        setNotice({
          text: t("새 그룹 ‘{0}’ 으로 페이지 {1}개를 복원했습니다.", input.name, newGroupRestore.linkIDs.length),
          groupID: restored.groupID,
        });
      }}
    />`}
    ${pendingDelete &&
    html`<${Confirm}
      title=${pendingDelete.kind === "group" ? t("보관한 그룹을 영구 삭제할까요?") : pendingDelete.kind === "groupLink" ? t("보관 그룹 안의 이 페이지를 영구 삭제할까요?") : t("보관한 페이지를 영구 삭제할까요?")}
      message=${t("보관함에서도 지웁니다. 앱을 끄기 전에는 {0} 로 되돌릴 수 있습니다.", UNDO_KEY)}
      confirmLabel=${t("영구 삭제")}
      onConfirm=${deleteNow}
      onCancel=${() => setPendingDelete(null)}
    />`}
  </div>`;
}
