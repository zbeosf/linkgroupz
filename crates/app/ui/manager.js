// 관리 창: 왼쪽 그룹 목록(검색 · 새 그룹 · 가져오기 · 보관함) + 오른쪽 그룹 화면(페이지 목록)

import { ArchiveView } from "./archive.js";
import { ImportTabGroupsSheet, ImportTabsSheet } from "./import.js";
import {
  afterPaint,
  boot,
  BrowserIcon,
  browserInfo,
  browserMenuItems,
  Check,
  classes,
  colorCss,
  Confirm,
  Empty,
  Favicon,
  GroupCardContent,
  GroupIcon,
  Highlight,
  html,
  Icon,
  IncognitoIcon,
  invoke,
  isComposingKey,
  linkName,
  listen,
  MenuButton,
  MiniSwitch,
  ProfileSwitcher,
  render,
  SearchField,
  SearchScopeButton,
  searchTerms,
  SelectAll,
  showMenu,
  sidebarItems,
  t,
  TagChips,
  toggled,
  UNDO_KEY,
  useEffect,
  useFiltered,
  useProfiles,
  useRef,
  useState,
  useStore,
  useUndoKeys,
} from "./lib/ui.js";
import { GroupEditSheet, LinkEditSheet, SectionNameSheet } from "./sheets.js";
import { UpdateBadge } from "./update-ui.js";
import { GroupSyncSheet } from "./sync.js";
import { archiveDrag, setArchiveDrag } from "./lib/drag.js";

/** 끌고 있는 것. 끄는 동안에는 내용(dataTransfer)을 읽을 수 없어 따로 기억한다. */
let dragging = null; // { kind: "groups", ids } | { kind: "section", id } | { kind: "links", groupID, ids }

/** 마우스가 줄의 위쪽 절반에 있으면 그 줄 앞, 아래쪽이면 그 줄 뒤에 끼운다. */
function insertionIndex(event, index) {
  const box = event.currentTarget.getBoundingClientRect();
  return event.clientY < box.top + box.height / 2 ? index : index + 1;
}

/** 줄 안에서 마우스가 위에서부터 어디쯤인지 (0 ~ 1) */
function verticalFraction(event) {
  const box = event.currentTarget.getBoundingClientRect();
  return (event.clientY - box.top) / Math.max(1, box.height);
}

/** 왼쪽 목록을 접어 두었는지 (이 컴퓨터에만 기억한다) */
function savedSidebarHidden() {
  try {
    return localStorage.getItem("sidebarHidden") === "1";
  } catch {
    return false;
  }
}

function saveSidebarHidden(hidden) {
  try {
    localStorage.setItem("sidebarHidden", hidden ? "1" : "0");
  } catch {
    // 저장소를 쓸 수 없으면 이번에만 접는다.
  }
}

function Manager() {
  const store = useStore();
  const scene = boot.scene;

  const [selection, setSelection] = useState(null);
  // 사용법 스크린샷용: scene=search:<검색어>
  const [query, setQuery] = useState(() => (scene?.startsWith("search:") ? scene.slice("search:".length) : ""));
  const [groupSheet, setGroupSheet] = useState(null); // { original } (없으면 새 그룹)
  const [linkSheet, setLinkSheet] = useState(null); // { kind: "new" | "edit" | "importTabs", groupID, link }
  const [pendingDelete, setPendingDelete] = useState(null); // { kind: "group" | "groups" | "links", ... }
  const [previewLinkID, setPreviewLinkID] = useState(null);
  const [importingTabGroups, setImportingTabGroups] = useState(false);
  const [showingArchive, setShowingArchive] = useState(false);

  // 목록 편집
  const [editingGroups, setEditingGroups] = useState(false);
  const [checkedGroups, setCheckedGroups] = useState(new Set());
  const [editingLinks, setEditingLinks] = useState(false);
  const [checkedLinks, setCheckedLinks] = useState(new Set());
  const [linkSelection, setLinkSelection] = useState(new Set()); // 편집 중이 아닐 때 목록 선택
  const [anchorLink, setAnchorLink] = useState(null); // Shift 로 이어 고를 때의 시작 줄
  const [dropTargetGroup, setDropTargetGroup] = useState(null);
  const [groupDrop, setGroupDrop] = useState(null); // { key, position: "before" | "after" | "onto" }
  const [linkInsert, setLinkInsert] = useState(null);
  // 그룹 여러 개 고르기 (⌘ · Shift 클릭)
  const [groupSelection, setGroupSelection] = useState(new Set());
  const [anchorGroup, setAnchorGroup] = useState(null);
  const [sectionSheet, setSectionSheet] = useState(null); // { kind: "new", at, groupIDs } | { kind: "rename", section }
  const [sidebarHidden, setSidebarHidden] = useState(savedSidebarHidden);
  const [notice, setNotice] = useState(null);
  // 그룹 안 검색 ('이 그룹에서 찾기')
  const [groupQuery, setGroupQuery] = useState("");
  // 그룹 동기화 중인 그룹
  const [syncingGroup, setSyncingGroup] = useState(null);
  // 보관함 버튼 위로 무언가를 끌고 있는지
  const [archiveDropping, setArchiveDropping] = useState(false);
  // 키보드로 그룹 목록의 섹션 머리줄에 와 있으면 그 섹션 id (그룹에 와 있으면 null — 고른 그룹이 곧 위치)
  const [sectionCursor, setSectionCursor] = useState(null);

  const filtering = query.trim() !== "";
  const visibleGroups = useFiltered("filter_groups", query, store, store.groups);
  const terms = filtering ? searchTerms(query) : null;
  const sectionByID = (id) => store.sections.find((section) => section.id === id);
  const groupByID = (id) => store.groups.find((group) => group.id === id);
  const selectedGroup = selection ? groupByID(selection) : null;
  // 그룹 안 검색: 맞는 페이지 id (검색어가 없으면 null)
  const groupMatches = useFiltered("filter_group_links", selectedGroup ? groupQuery : "", store, null, { groupId: selection });
  const searchingGroup = groupQuery.trim() !== "" && Boolean(selectedGroup);
  const groupTerms = searchingGroup ? searchTerms(groupQuery) : null;
  // 검색으로 걸러진 페이지만 보여준다 (왼쪽 검색 · 그룹 안 검색 모두).
  const shownLinks = selectedGroup
    ? (visibleGroups.find((group) => group.id === selectedGroup.id)?.links ?? []).filter((link) => !searchingGroup || !groupMatches || groupMatches.includes(link.id))
    : [];
  // 페이지 줄에서 강조할 검색어
  const linkTerms = terms || groupTerms ? { text: [...(terms?.text ?? []), ...(groupTerms?.text ?? [])] } : null;
  // 걸러서 보이는 중에는 끌어서 순서를 바꾸지 않는다.
  const filteringLinks = filtering || searchingGroup;

  const closePreview = () => {
    setPreviewLinkID((current) => {
      if (current) {
        invoke("close_preview");
      }
      return null;
    });
  };

  const select = (id) => {
    if (id === selection) {
      return;
    }
    setSelection(id);
    setGroupQuery("");
    if (id) {
      setShowingArchive(false);
    }
    setEditingLinks(false);
    setCheckedLinks(new Set());
    setLinkSelection(new Set());
    setAnchorLink(null);
    closePreview();
  };

  // 프로필을 바꾸면 고른 그룹 · 편집 · 검색 상태를 처음부터 (처음 그릴 때는 그대로 둔다).
  const profileID = useProfiles().current;
  const shownProfile = useRef(profileID);
  useEffect(() => {
    if (shownProfile.current === profileID) {
      return;
    }
    shownProfile.current = profileID;
    select(null);
    setShowingArchive(false);
    setEditingGroups(false);
    setGroupSelection(new Set());
    setAnchorGroup(null);
    setQuery("");
  }, [profileID]);

  const showNotice = (message) => {
    setNotice(message);
    setTimeout(() => setNotice((current) => (current === message ? null : current)), 3000);
  };
  useUndoKeys(showNotice);

  // 작은 미리보기가 닫히면 눈 버튼을 되돌린다.
  useEffect(
    () =>
      listen("preview-closed", (info) => {
        if (info.compact) {
          setPreviewLinkID(null);
        }
      }),
    [],
  );

  // 사용법 스크린샷용 화면 상태
  useEffect(() => {
    if (!scene) {
      return;
    }
    if (scene === "addGroup") {
      invoke("add_group", {
        input: {
          name: t("새로 추가한 그룹 {0}", store.groups.length + 1),
          color: "green",
          tags: [],
          browser: boot.browsers[0].id,
          icon: { symbol: boot.defaultSymbol },
          alwaysPrivate: false,
        },
      }).then((id) => invoke("add_link", { groupId: id, url: "https://example.com", alias: t("예시") }));
      return;
    }
    const first = store.groups[0];
    if (!first) {
      return;
    }
    setSelection(first.id);
    setTimeout(() => {
      if (scene === "edit") {
        setEditingLinks(true);
        setCheckedLinks(new Set(first.links.slice(1, 3).map((link) => link.id)));
      } else if (scene === "importTabs") {
        setLinkSheet({ kind: "importTabs", groupID: first.id });
      } else if (scene === "importGroups" || scene === "importClosedGroups") {
        setImportingTabGroups(true);
      } else if (scene === "groupEdit") {
        setGroupSheet({ original: first });
      } else if (scene === "settings") {
        invoke("show_settings");
      } else if (scene === "preview" && first.links.length > 1) {
        const row = document.querySelector(`[data-link="${first.links[1].id}"]`);
        if (row) {
          togglePreview(first, first.links[1], row);
        }
      } else if (scene.startsWith("archive")) {
        setSelection(null);
        setShowingArchive(true);
      } else if (scene.startsWith("profile:")) {
        invoke("switch_profile", { id: scene.slice("profile:".length) });
      }
    }, 600);
  }, []);

  // MARK: 메뉴바 창에서 넘어온 일 (편집 · 페이지 추가 · 삭제 확인처럼 입력 · 확인이 필요한 것)

  const runManagerAction = (action) => {
    if (!action) {
      return;
    }
    const group = groupByID(action.groupId ?? action.id);
    const section = sectionByID(action.sectionId ?? action.id);
    const link = group?.links.find((item) => item.id === action.linkId);
    if (group && action.kind !== "editSection" && action.kind !== "deleteSection" && action.kind !== "newGroupInSection") {
      select(group.id);
      setGroupSelection(new Set([group.id]));
      setAnchorGroup(group.id);
    }
    switch (action.kind) {
      case "editGroup":
        group && setGroupSheet({ original: group });
        break;
      case "addLink":
        group && setLinkSheet({ kind: "new", groupID: group.id });
        break;
      case "importTabs":
        group && setLinkSheet({ kind: "importTabs", groupID: group.id });
        break;
      case "syncGroup":
        group && setSyncingGroup(group.id);
        break;
      case "newSection":
        group && setSectionSheet({ kind: "new", at: group.id, groupIDs: [group.id] });
        break;
      case "deleteGroup":
        group && setPendingDelete({ kind: "group", group });
        break;
      case "editLink":
        link && setLinkSheet({ kind: "edit", groupID: group.id, link });
        break;
      case "previewLink":
        if (link) {
          setLinkSelection(new Set([link.id]));
          // 그룹 화면이 그려진 뒤 그 줄 옆에 띄운다.
          setTimeout(() => {
            const row = document.querySelector(`[data-link="${link.id}"]`);
            if (row) {
              togglePreview(group, link, row);
            }
          }, 300);
        }
        break;
      case "deleteLinks":
        group && setPendingDelete({ kind: "links", groupID: group.id, ids: new Set(action.ids ?? []) });
        break;
      case "newGroupInSection":
        section && setGroupSheet({ section: section.id });
        break;
      case "editSection":
        section && setSectionSheet({ kind: "rename", section });
        break;
      case "deleteSection":
        section && setPendingDelete({ kind: "section", section });
        break;
    }
  };
  // 늘 지금 화면 상태로 처리하도록 처리 함수만 바꿔 끼운다.
  const managerActionHandler = useRef(runManagerAction);
  managerActionHandler.current = runManagerAction;
  useEffect(() => {
    const take = () => invoke("take_manager_action").then((action) => managerActionHandler.current(action));
    take();
    return listen("manager-action", take);
  }, []);

  // MARK: 그룹 다루기

  const openGroup = (group, newWindow = false) => invoke("open_group", { id: group.id, newWindow });
  const archiveGroups = (ids) => {
    if (ids.size === 0) {
      return;
    }
    invoke("archive_groups", { ids: [...ids] });
    if (selection && ids.has(selection)) {
      select(null);
    }
    setCheckedGroups(new Set([...checkedGroups].filter((id) => !ids.has(id))));
    setGroupSelection(new Set([...groupSelection].filter((id) => !ids.has(id))));
    if (editingGroups && store.groups.length === ids.size) {
      setEditingGroups(false);
    }
  };

  /** 그룹들을 섹션에 넣거나(sectionID) 섹션 밖으로(null) 옮긴다 */
  const moveToSection = (ids, sectionID) => invoke("move_items", { ids: [...ids], target: { section: sectionID } });

  /** '섹션' 메뉴: 섹션들 · 섹션 없음 · 새 섹션 */
  const sectionMenu = (ids, current) => ({
    label: t("섹션"),
    submenu: [
      ...store.sections.map((section) => ({ label: section.name, checked: current === section.id, action: () => moveToSection(ids, section.id) })),
      ...(store.sections.length > 0 ? ["-"] : []),
      { label: t("섹션 없음"), checked: current === null, action: () => moveToSection(ids, null) },
      { label: t("새 섹션 만들기…"), action: () => setSectionSheet({ kind: "new", at: [...ids][0], groupIDs: [...ids] }) },
    ],
  });

  const groupMenu = (group) => [
    { label: t("그룹 열기"), disabled: group.links.length === 0, action: () => openGroup(group) },
    { label: t("새 창으로 열기"), disabled: group.links.length === 0, action: () => openGroup(group, true) },
    { label: t("시크릿 모드로 열기"), disabled: group.links.length === 0, action: () => invoke("open_group_private", { id: group.id }) },
    { label: t("페이지 추가"), action: () => setLinkSheet({ kind: "new", groupID: group.id }) },
    { label: t("열린 탭 가져오기"), action: () => setLinkSheet({ kind: "importTabs", groupID: group.id }) },
    { label: t("브라우저 탭 그룹과 동기화…"), action: () => setSyncingGroup(group.id) },
    { label: t("브라우저"), submenu: browserMenuItems(group.browser, (browser) => invoke("set_group_browser", { id: group.id, browser })) },
    {
      label: t("항상 시크릿 모드로 열기"),
      checked: group.alwaysPrivate,
      action: () => invoke("set_group_always_private", { id: group.id, alwaysPrivate: !group.alwaysPrivate }),
    },
    { label: t("편집"), action: () => setGroupSheet({ original: group }) },
    sectionMenu(new Set([group.id]), group.sectionID ?? null),
    { label: t("새 섹션 만들기"), action: () => setSectionSheet({ kind: "new", at: group.id, groupIDs: [group.id] }) },
    "-",
    { label: t("보관함으로 이동"), action: () => archiveGroups(new Set([group.id])) },
    { label: t("삭제"), destructive: true, action: () => setPendingDelete({ kind: "group", group }) },
  ];
  /** 여러 그룹을 고른 채 오른쪽 클릭 */
  const groupsMenu = (ids) => [
    sectionMenu(ids, undefined),
    "-",
    { label: t("{0}개 보관함으로 이동", ids.size), action: () => archiveGroups(ids) },
    { label: t("{0}개 삭제", ids.size), destructive: true, action: () => setPendingDelete({ kind: "groups", ids }) },
  ];
  const listMenu = [
    { label: t("새 그룹 만들기"), action: () => setGroupSheet({}) },
    { label: t("새 섹션 만들기"), action: () => setSectionSheet({ kind: "new", at: null, groupIDs: [] }) },
    { label: t("탭 그룹 가져오기"), action: () => setImportingTabGroups(true) },
  ];
  const sectionContextMenu = (section) => {
    const index = (store.sidebarOrder ?? []).indexOf(`section:${section.id}`);
    const outside = store.groups.filter((group) => group.sectionID !== section.id);
    return [
      {
        label: t("그룹 추가"),
        disabled: outside.length === 0,
        submenu: outside.map((group) => ({ label: group.name, action: () => moveToSection([group.id], section.id) })),
      },
      { label: t("새 그룹 만들기"), action: () => setGroupSheet({ section: section.id }) },
      { label: t("섹션 편집…"), action: () => setSectionSheet({ kind: "rename", section }) },
      { label: t("위로 이동"), disabled: index <= 0, action: () => invoke("move_section", { id: section.id, offset: -1 }) },
      {
        label: t("아래로 이동"),
        disabled: index < 0 || index >= (store.sidebarOrder ?? []).length - 1,
        action: () => invoke("move_section", { id: section.id, offset: 1 }),
      },
      "-",
      { label: t("섹션 삭제…"), destructive: true, action: () => setPendingDelete({ kind: "section", section }) },
    ];
  };

  // MARK: 페이지 다루기

  const openLinks = (group, ids, isPrivate = false, newWindow = false) =>
    invoke("open_links", { groupId: group.id, ids: [...ids], private: isPrivate, newWindow });
  const archiveLinks = (group, ids) => {
    if (ids.size === 0) {
      return;
    }
    invoke("archive_links", { groupId: group.id, ids: [...ids] });
    closePreview();
    setLinkSelection(new Set([...linkSelection].filter((id) => !ids.has(id))));
    setCheckedLinks(new Set([...checkedLinks].filter((id) => !ids.has(id))));
  };
  const moveLinks = async (ids, sourceID, targetID) => {
    const result = await invoke("move_links", { ids: [...ids], sourceId: sourceID, targetId: targetID });
    setCheckedLinks((current) => new Set([...current].filter((id) => !ids.has(id))));
    let message = t("‘{0}’ 으로 {1}개 옮겼습니다", groupByID(targetID)?.name ?? "", result.moved);
    if (result.duplicates > 0) {
      message += t(" (이미 있던 {0}개는 원래 그룹에서만 뺌)", result.duplicates);
    }
    showNotice(message);
  };
  const moveMenu = (group, ids) => {
    const targets = store.groups.filter((other) => other.id !== group.id);
    return {
      label: t("다른 그룹으로 이동"),
      disabled: targets.length === 0 || ids.size === 0,
      submenu: targets.map((target) => ({ label: target.name, action: () => moveLinks(ids, group.id, target.id) })),
    };
  };

  async function togglePreview(group, link, row) {
    if (previewLinkID === link.id) {
      closePreview();
      return;
    }
    const box = row.getBoundingClientRect();
    const anchor = { left: box.left, top: box.top, right: box.right, bottom: box.bottom };
    setLinkSelection(new Set([link.id]));
    try {
      const opened = await invoke("toggle_preview", { groupId: group.id, linkId: link.id, anchor });
      setPreviewLinkID(opened ? link.id : null);
    } catch (message) {
      setPreviewLinkID(null);
      showNotice(String(message));
    }
  }

  const linkMenu = (group, ids, rowOf) => {
    const links = group.links.filter((link) => ids.has(link.id));
    const idSet = new Set(links.map((link) => link.id));
    if (links.length === 0) {
      return [];
    }
    const single = links.length === 1 ? links[0] : null;
    return [
      ...(single
        ? [
            { label: t("열기"), action: () => openLinks(group, idSet) },
            { label: t("새 창으로 열기"), action: () => openLinks(group, idSet, false, true) },
            { label: t("시크릿 모드로 열기"), action: () => openLinks(group, idSet, true) },
            { label: t("미리보기"), action: () => togglePreview(group, single, rowOf(single.id)) },
            { label: t("편집"), action: () => setLinkSheet({ kind: "edit", groupID: group.id, link: single }) },
            { label: t("URL 복사"), action: () => invoke("copy_text", { text: single.url }) },
          ]
        : [
            { label: t("{0}개 열기", links.length), action: () => openLinks(group, idSet) },
            { label: t("{0}개 새 창으로 열기", links.length), action: () => openLinks(group, idSet, false, true) },
            { label: t("{0}개 시크릿 모드로 열기", links.length), action: () => openLinks(group, idSet, true) },
          ]),
      moveMenu(group, idSet),
      "-",
      { label: single ? t("보관함으로 이동") : t("{0}개 보관함으로 이동", links.length), action: () => archiveLinks(group, idSet) },
      {
        label: single ? t("삭제") : t("{0}개 삭제", links.length),
        destructive: true,
        action: () => setPendingDelete({ kind: "links", groupID: group.id, ids: idSet }),
      },
    ];
  };

  // MARK: 삭제 확인

  let deleteTitle = "";
  let deleteMessage = "";
  if (pendingDelete?.kind === "group") {
    deleteTitle = t("‘{0}’ 그룹을 삭제할까요?", pendingDelete.group.name);
    deleteMessage = t("그룹 안의 페이지 {0}개도 함께 삭제됩니다.", pendingDelete.group.links.length);
  } else if (pendingDelete?.kind === "groups") {
    const pages = store.groups.filter((group) => pendingDelete.ids.has(group.id)).reduce((sum, group) => sum + group.links.length, 0);
    deleteTitle = t("그룹 {0}개를 삭제할까요?", pendingDelete.ids.size);
    deleteMessage = t("그룹 안의 페이지 {0}개도 함께 삭제됩니다.", pages);
  } else if (pendingDelete?.kind === "links") {
    deleteTitle = t("페이지 {0}개를 삭제할까요?", pendingDelete.ids.size);
    deleteMessage = t("앱을 끄기 전에는 {0} 로 되돌릴 수 있습니다.", UNDO_KEY);
  } else if (pendingDelete?.kind === "section") {
    deleteTitle = t("‘{0}’ 섹션을 삭제할까요?", pendingDelete.section.name);
    deleteMessage = t("안의 그룹은 지우지 않고 섹션이 있던 자리에 남깁니다.");
  }
  const performDelete = () => {
    if (pendingDelete.kind === "group") {
      invoke("delete_groups", { ids: [pendingDelete.group.id] });
      if (selection === pendingDelete.group.id) {
        select(null);
      }
    } else if (pendingDelete.kind === "groups") {
      invoke("delete_groups", { ids: [...pendingDelete.ids] });
      if (selection && pendingDelete.ids.has(selection)) {
        select(null);
      }
      setCheckedGroups(new Set());
      setGroupSelection(new Set());
      setEditingGroups(false);
    } else if (pendingDelete.kind === "section") {
      invoke("delete_section", { id: pendingDelete.section.id });
    } else {
      invoke("delete_links", { groupId: pendingDelete.groupID, ids: [...pendingDelete.ids] });
      setCheckedLinks(new Set());
      setLinkSelection(new Set([...linkSelection].filter((id) => !pendingDelete.ids.has(id))));
    }
    setPendingDelete(null);
  };

  // MARK: 사이드바

  // 화면에 보이는 줄: 섹션 머리줄 · 그룹 · 펼친 섹션 끝의 가로줄
  const items = sidebarItems(store, filtering ? visibleGroups : null);
  const rows = [];
  for (const item of items) {
    if (item.kind === "group") {
      rows.push({ kind: "group", key: item.group.id, group: item.group, sectionID: null });
      continue;
    }
    const { section, groups } = item;
    const expanded = filtering || !section.isCollapsed;
    rows.push({ kind: "section", key: section.id, section, count: groups.length, expanded });
    if (expanded) {
      groups.forEach((group, index) => rows.push({ kind: "group", key: group.id, group, sectionID: section.id, last: index === groups.length - 1 }));
      rows.push({ kind: "section-end", key: `end-${section.id}`, section });
    }
  }
  const flatGroups = rows.filter((row) => row.kind === "group").map((row) => row.group);

  const clearDrag = () => {
    dragging = null;
    setArchiveDrag(null);
    setGroupDrop(null);
    setDropTargetGroup(null);
    setArchiveDropping(false);
  };
  const draggingGroups = () => dragging?.kind === "groups";
  const draggingSection = () => dragging?.kind === "section";
  // 보관함에서 끌어 온 것: 보관 그룹은 그 자리에 복원, 보관 페이지 · 보관 그룹 안 페이지는 그룹 위에 놓아 복원
  const draggingArchivedGroups = () => archiveDrag?.kind === "archivedGroups";
  const draggingArchivedLinks = () => archiveDrag?.kind === "archivedLinks" || archiveDrag?.kind === "archivedGroupLinks";
  const restoreArchivedLinksTo = async (group) => {
    const drag = archiveDrag;
    const result =
      drag.kind === "archivedLinks"
        ? await invoke("restore_links", { ids: drag.ids, targetId: group.id })
        : await invoke("restore_archived_group_links", { groupId: drag.groupId, ids: drag.ids, targetId: group.id });
    showNotice(result.duplicates > 0 ? t("‘{0}’ 에 같은 주소가 이미 있어 보관함에서만 뺐습니다", group.name) : t("‘{0}’ 에 복원했습니다", group.name));
  };

  const clickGroup = (event, group) => {
    setSectionCursor(null);
    if (editingGroups) {
      setCheckedGroups(toggled(checkedGroups, group.id));
      return;
    }
    if (event.shiftKey && anchorGroup) {
      // 시작 그룹부터 이 그룹까지 이어서 고른다.
      const ids = flatGroups.map((item) => item.id);
      const [from, to] = [ids.indexOf(anchorGroup), ids.indexOf(group.id)].sort((a, b) => a - b);
      setGroupSelection(new Set(ids.slice(Math.max(0, from), to + 1)));
      select(group.id);
      return;
    }
    if (event.metaKey || event.ctrlKey) {
      const next = toggled(groupSelection.size === 0 && selection ? new Set([selection]) : groupSelection, group.id);
      setGroupSelection(next);
      setAnchorGroup(group.id);
      if (next.has(group.id)) {
        select(group.id);
      }
      return;
    }
    setGroupSelection(new Set([group.id]));
    setAnchorGroup(group.id);
    select(group.id);
  };

  /** 끌어다 놓기: 그룹 · 섹션을 옮긴다 (보관 그룹이면 그 자리에 복원) */
  const dropItems = (target) => {
    if (draggingArchivedGroups()) {
      invoke("restore_groups_to", { ids: archiveDrag.ids, target });
      showNotice(t("보관한 그룹을 복원했습니다"));
    } else if (draggingGroups()) {
      invoke("move_items", { ids: dragging.ids, target });
    } else if (draggingSection()) {
      invoke("move_items", { ids: [dragging.id], target: { section: null, before: target.before ?? null, after: target.after ?? null } });
    }
    clearDrag();
  };

  /** 이 그룹 뒤 자리: 섹션 안이면 다음 그룹 앞(없으면 섹션 끝), 맨 위 단계면 이 그룹 바로 뒤 */
  const afterGroup = (row) => {
    if (!row.sectionID) {
      return { section: null, after: row.group.id };
    }
    const inSection = store.groups.filter((group) => group.sectionID === row.sectionID);
    const next = inSection[inSection.findIndex((group) => group.id === row.group.id) + 1];
    return next ? { section: row.sectionID, before: next.id } : { section: row.sectionID };
  };

  const groupRow = (row) => {
    const { group } = row;
    const full = groupByID(group.id) ?? group;
    const selected = selection === group.id || checkedGroups.has(group.id);
    // 원작처럼 고른 카드(하나만 골라도)는 왼쪽 위 모서리에 파란 삼각형을 단다. 목록 편집 중에는 체크박스가 대신한다.
    const marked = !editingGroups && (selection === group.id || groupSelection.has(group.id));
    const drop = groupDrop?.key === row.key ? groupDrop.position : null;
    const linksDroppable = () => (dragging?.kind === "links" && dragging.groupID !== group.id) || draggingArchivedLinks();
    const groupsDroppable = () => ((draggingGroups() && !dragging.ids.includes(group.id)) || draggingArchivedGroups()) && !filtering;
    const sectionDroppable = () => draggingSection() && !row.sectionID && !filtering;
    const position = (event) => {
      const fraction = verticalFraction(event);
      if (draggingGroups() && groupsDroppable() && !row.sectionID && fraction > 0.3 && fraction < 0.7) {
        return "onto"; // 두 그룹을 담은 섹션을 만든다
      }
      return fraction < 0.5 ? "before" : "after";
    };
    return html`<div
      key=${row.key}
      data-row=${`g:${group.id}`}
      class=${classes("group-row", row.sectionID && "in-section", drop === "before" && "insert-before", drop === "after" && "insert-after")}
      draggable=${!filtering && !editingGroups}
      onDragStart=${(event) => {
        // 고른 그룹을 끌면 고른 그룹을 모두 옮긴다.
        const ids = groupSelection.has(group.id) && groupSelection.size > 1 ? flatGroups.map((item) => item.id).filter((id) => groupSelection.has(id)) : [group.id];
        dragging = { kind: "groups", ids };
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", group.name);
      }}
      onDragEnd=${clearDrag}
      onDragOver=${(event) => {
        if (linksDroppable()) {
          event.preventDefault();
          event.dataTransfer.dropEffect = "move";
          setDropTargetGroup(group.id);
        } else if (groupsDroppable() || sectionDroppable()) {
          event.preventDefault();
          event.dataTransfer.dropEffect = "move";
          setGroupDrop({ key: row.key, position: position(event) });
        }
      }}
      onDragLeave=${() => {
        if (dropTargetGroup === group.id) {
          setDropTargetGroup(null);
        }
      }}
      onDrop=${(event) => {
        event.preventDefault();
        if (linksDroppable()) {
          // 페이지를 이 그룹으로 옮긴다 (보관함의 페이지면 이 그룹에 복원).
          if (draggingArchivedLinks()) {
            restoreArchivedLinksTo(full);
          } else {
            moveLinks(new Set(dragging.ids), dragging.groupID, group.id);
          }
          clearDrag();
          return;
        }
        if (!groupsDroppable() && !sectionDroppable()) {
          clearDrag();
          return;
        }
        const where = position(event);
        if (where === "onto") {
          // 그룹 위에 그룹을 놓으면 두 그룹을 담은 섹션을 그 자리에 만든다.
          setSectionSheet({ kind: "new", at: group.id, groupIDs: [group.id, ...dragging.ids] });
          clearDrag();
        } else if (where === "before") {
          dropItems({ section: row.sectionID, before: group.id });
        } else {
          dropItems(afterGroup(row));
        }
      }}
    >
      ${editingGroups &&
      html`<${Check} state=${checkedGroups.has(group.id) ? "on" : "off"} onToggle=${() => setCheckedGroups(toggled(checkedGroups, group.id))} />`}
      <div
        class=${classes("group-card", "hoverable", selected && "selected", marked && "marked", (dropTargetGroup === group.id || drop === "onto") && "drop-target")}
        style=${`--tint:${colorCss(group.color)}`}
        onClick=${(event) => clickGroup(event, group)}
        onDblClick=${(event) => {
          // 더블클릭 → 그룹 열기 (목록 편집 · 여러 개 고르는 중에는 열지 않는다)
          if (!editingGroups && !event.metaKey && !event.ctrlKey && !event.shiftKey && full.links.length > 0) {
            openGroup(full);
          }
        }}
        onContextMenu=${(event) => {
          event.stopPropagation();
          if (groupSelection.size > 1 && groupSelection.has(group.id) && !editingGroups) {
            showMenu(event, groupsMenu(new Set(groupSelection)));
            return;
          }
          setGroupSelection(new Set([group.id]));
          setAnchorGroup(group.id);
          select(group.id);
          showMenu(event, [...groupMenu(full), "-", ...listMenu]);
        }}
      >
        <${GroupCardContent} group=${group} linkCount=${full.links.length} terms=${terms} />
      </div>
    </div>`;
  };

  const sectionRow = (row) => {
    const { section } = row;
    const drop = groupDrop?.key === row.key ? groupDrop.position : null;
    const position = (event) => {
      const fraction = verticalFraction(event);
      if (draggingGroups() || draggingArchivedGroups()) {
        return fraction < 0.25 ? "before" : "onto"; // 머리줄에 놓으면 섹션 끝에 넣는다
      }
      return fraction < 0.5 ? "before" : "after";
    };
    const droppable = () => !filtering && (draggingGroups() || draggingArchivedGroups() || (draggingSection() && dragging.id !== section.id));
    const toggle = () => {
      if (!filtering) {
        invoke("set_section_collapsed", { id: section.id, collapsed: !section.isCollapsed });
      }
    };
    return html`<div
      key=${row.key}
      data-row=${`s:${section.id}`}
      class=${classes("section-row", sectionCursor === section.id && "keyboard", drop === "before" && "insert-before", drop === "after" && "insert-after", drop === "onto" && "drop-target")}
      draggable=${!filtering && !editingGroups}
      title=${row.expanded ? t("누르면 접기 · 끌어서 순서 변경") : t("누르면 펼치기 · 끌어서 순서 변경")}
      onClick=${toggle}
      onDragStart=${(event) => {
        dragging = { kind: "section", id: section.id };
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", section.name);
      }}
      onDragEnd=${clearDrag}
      onDragOver=${(event) => {
        if (droppable()) {
          event.preventDefault();
          event.dataTransfer.dropEffect = "move";
          setGroupDrop({ key: row.key, position: position(event) });
        }
      }}
      onDrop=${(event) => {
        event.preventDefault();
        if (!droppable()) {
          clearDrag();
          return;
        }
        const where = position(event);
        if (where === "onto") {
          dropItems({ section: section.id });
        } else if (where === "before") {
          dropItems({ section: null, before: section.id });
        } else {
          dropItems({ section: null, after: section.id });
        }
      }}
      onContextMenu=${(event) => {
        event.stopPropagation();
        showMenu(event, [...sectionContextMenu(section), "-", ...listMenu]);
      }}
    >
      <${Icon} name=${row.expanded ? "caret-down-bold" : "caret-right-bold"} size=${11} class="chevron-icon" />
      <span class="name truncate"><${Highlight} text=${section.name} terms=${terms?.text} /></span>
      <span class="count">${row.count}</span>
    </div>`;
  };

  /** 펼친 섹션 끝의 가로줄. 여기에 놓으면 섹션 밖, 섹션 바로 뒤로 간다. */
  const sectionEndRow = (row) => {
    const drop = groupDrop?.key === row.key;
    const droppable = () => !filtering && (draggingGroups() || draggingSection() || draggingArchivedGroups());
    return html`<div
      key=${row.key}
      class=${classes("section-end", drop && "insert-after")}
      onDragOver=${(event) => {
        if (droppable()) {
          event.preventDefault();
          event.dataTransfer.dropEffect = "move";
          setGroupDrop({ key: row.key, position: "after" });
        }
      }}
      onDrop=${(event) => {
        event.preventDefault();
        if (droppable()) {
          dropItems({ section: null, after: row.section.id });
        } else {
          clearDrag();
        }
      }}
    ></div>`;
  };

  // MARK: 키보드 — 그룹 목록: ↑↓ 그룹 · 섹션, ←→ 섹션 접기 · 펼치기와 페이지 목록 오가기, Return 열기, Tab 메뉴

  /** 키보드로 오가는 줄 (섹션 머리줄 · 그룹) */
  const navRows = rows.filter((row) => row.kind === "group" || row.kind === "section");
  const cursorIndex = sectionCursor
    ? navRows.findIndex((row) => row.kind === "section" && row.section.id === sectionCursor)
    : navRows.findIndex((row) => row.kind === "group" && row.group.id === selection);
  const rowElement = (key) => document.querySelector(`[data-row="${key}"]`);

  const moveCursorTo = (row) => {
    if (!row) {
      return;
    }
    if (row.kind === "section") {
      setSectionCursor(row.section.id);
      rowElement(`s:${row.section.id}`)?.scrollIntoView({ block: "nearest" });
      return;
    }
    setSectionCursor(null);
    setGroupSelection(new Set([row.group.id]));
    setAnchorGroup(row.group.id);
    select(row.group.id);
    rowElement(`g:${row.group.id}`)?.scrollIntoView({ block: "nearest" });
  };

  const focusGroupList = () => document.querySelector(".group-list")?.focus();
  const focusLinkList = () => {
    document.querySelector(".link-list")?.focus();
    if (linkSelection.size === 0 && shownLinks[0]) {
      setLinkSelection(new Set([shownLinks[0].id]));
      setAnchorLink(shownLinks[0].id);
    }
  };

  /** 지금 위치(섹션 · 그룹)의 오른쪽 클릭 메뉴를 그 줄 아래에 띄운다 (Tab) */
  const showCursorMenu = () => {
    const row = navRows[cursorIndex];
    if (row?.kind === "section") {
      showMenu(rowElement(`s:${row.section.id}`), [...sectionContextMenu(row.section), "-", ...listMenu]);
    } else if (row?.kind === "group") {
      const full = groupByID(row.group.id) ?? row.group;
      showMenu(rowElement(`g:${row.group.id}`)?.querySelector(".group-card") ?? rowElement(`g:${row.group.id}`), [...groupMenu(full), "-", ...listMenu]);
    } else {
      showMenu(document.querySelector(".group-list"), listMenu);
    }
  };

  const groupListKey = (event) => {
    if (isComposingKey(event) || editingGroups) {
      return;
    }
    const row = navRows[cursorIndex];
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const step = event.key === "ArrowDown" ? 1 : -1;
      moveCursorTo(navRows[cursorIndex < 0 ? (step > 0 ? 0 : navRows.length - 1) : Math.max(0, Math.min(navRows.length - 1, cursorIndex + step))]);
    } else if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault();
      const open = event.key === "ArrowRight";
      if (row?.kind === "section") {
        if (!filtering && row.section.isCollapsed === open) {
          invoke("set_section_collapsed", { id: row.section.id, collapsed: !open });
        }
      } else if (row?.kind === "group") {
        if (open) {
          focusLinkList();
        } else if (row.sectionID) {
          // 섹션 안 그룹에서 ← 는 그 섹션 머리줄로
          moveCursorTo(navRows.find((other) => other.kind === "section" && other.section.id === row.sectionID));
        }
      }
    } else if (event.key === "Enter") {
      if (row?.kind === "section" && !filtering) {
        invoke("set_section_collapsed", { id: row.section.id, collapsed: !row.section.isCollapsed });
      } else if (row?.kind === "group" && selectedGroup && selectedGroup.links.length > 0) {
        openGroup(selectedGroup);
      }
    } else if (event.key === "Tab") {
      event.preventDefault();
      showCursorMenu();
    } else if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "a") {
      event.preventDefault();
      setGroupSelection(new Set(flatGroups.map((group) => group.id)));
    }
  };

  // ⌘1 (Windows 는 Ctrl+1): 검색창으로
  useEffect(() => {
    const onKey = (event) => {
      if ((event.metaKey || event.ctrlKey) && event.key === "1" && !document.querySelector(".sheet-layer, .menu-layer")) {
        event.preventDefault();
        if (sidebarHidden) {
          setSidebarHidden(false);
          saveSidebarHidden(false);
        }
        setTimeout(() => {
          const input = document.querySelector('[data-search="sidebar"]');
          input?.focus();
          input?.select();
        });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  /** 검색창에서 ↓ 는 그룹 목록의 첫 줄로 */
  const searchKey = (event) => {
    if (event.key === "ArrowDown" && !isComposingKey(event) && navRows.length > 0) {
      event.preventDefault();
      focusGroupList();
      moveCursorTo(navRows[0]);
    }
  };

  const toggleSidebar = () => {
    setSidebarHidden(!sidebarHidden);
    saveSidebarHidden(!sidebarHidden);
  };

  const listEnd = groupDrop?.key === "list-end";
  const sidebar = html`<aside class="sidebar">
    <div class="profile-row" data-tauri-drag-region>
      <${ProfileSwitcher} />
      <span class="spacer" data-tauri-drag-region></span>
      <button type="button" class="btn plain" title=${t("목록 숨기기")} onClick=${toggleSidebar}><${Icon} name="sidebar-simple" size=${17} /></button>
    </div>
    <div class="top">
      <${SearchField} value=${query} onChange=${setQuery} placeholder=${t("이름 · 태그 · URL  ({0})", boot.platform === "macos" ? "⌘1" : "Ctrl+1")} onKeyDown=${searchKey} inputClass="sidebar" />
      <${SearchScopeButton} />
    </div>
    <div class="buttons">
      <button type="button" class="btn small" title=${t("새 그룹 만들기")} onClick=${() => setGroupSheet({})}>
        <${Icon} name="folder-plus" size=${14} />${t("새 그룹")}
      </button>
      <button type="button" class="btn small" title=${t("Chrome · Edge · Whale 의 탭 그룹을 그룹으로 가져오기")} onClick=${() => setImportingTabGroups(true)}>
        <${Icon} name="stack" size=${14} />${t("탭 그룹 가져오기")}
      </button>
    </div>
    <div class="summary">
      <span class="caption">${t("그룹 {0}개", store.groups.length)}</span>
      <span class="spacer"></span>
      <button
        type="button"
        class="btn small"
        title=${groupSelection.size > 1 ? t("고른 그룹 {0}개를 담은 섹션 만들기", groupSelection.size) : t("그룹을 묶을 섹션 만들기")}
        onClick=${() => setSectionSheet({ kind: "new", at: groupSelection.size > 1 ? flatGroups.find((group) => groupSelection.has(group.id))?.id ?? null : null, groupIDs: groupSelection.size > 1 ? [...groupSelection] : [] })}
      >
        <${Icon} name="rows" size=${14} />${t("섹션")}
      </button>
      <button
        type="button"
        class="btn small"
        disabled=${store.groups.length === 0 && !editingGroups}
        onClick=${() => {
          setEditingGroups(!editingGroups);
          setCheckedGroups(new Set());
          setGroupSelection(new Set());
        }}
      >${editingGroups ? t("완료") : t("목록 편집")}</button>
    </div>
    <div class="group-list" tabindex="0" onKeyDown=${groupListKey} onContextMenu=${(event) => showMenu(event, listMenu)}>
      ${store.groups.length === 0 && store.sections.length === 0
        ? html`<${Empty} icon="folder" title=${t("그룹이 없습니다")}>
            <div class="column" style="gap:6px">
              <button type="button" class="btn" onClick=${() => setGroupSheet({})}>${t("새 그룹 만들기")}</button>
              <button type="button" class="btn" onClick=${() => setImportingTabGroups(true)}>${t("탭 그룹 가져오기")}</button>
            </div>
          <//>`
        : filtering && visibleGroups.length === 0
          ? html`<${Empty} icon="magnifying-glass" title=${t("‘{0}’ 에 맞는 결과가 없습니다", query.trim())} description=${t("다른 검색어로 찾아보세요.")} />`
          : rows.map((row) => (row.kind === "group" ? groupRow(row) : row.kind === "section" ? sectionRow(row) : sectionEndRow(row)))}
      ${/* 목록 아래 빈 곳: 여기에 놓으면 맨 끝으로 간다. 오른쪽 클릭 메뉴도 뜬다. */ ""}
      <div
        class=${classes("list-end", listEnd && "insert-before")}
        onDragOver=${(event) => {
          if (!filtering && (draggingGroups() || draggingSection() || draggingArchivedGroups())) {
            event.preventDefault();
            event.dataTransfer.dropEffect = "move";
            setGroupDrop({ key: "list-end", position: "before" });
          }
        }}
        onDrop=${(event) => {
          event.preventDefault();
          if (!filtering && (draggingGroups() || draggingSection() || draggingArchivedGroups())) {
            dropItems({ section: null });
          } else {
            clearDrag();
          }
        }}
        onClick=${() => setGroupSelection(new Set())}
      ></div>
    </div>
    ${editingGroups &&
    html`<div class="divider"></div>
      <div class="edit-bar">
        <${SelectAll}
          selected=${checkedGroups.size}
          selectable=${visibleGroups.length}
          onSelectAll=${() => setCheckedGroups(new Set(visibleGroups.map((group) => group.id)))}
          onDeselectAll=${() => setCheckedGroups(new Set())}
        >${t("{0}개 선택", checkedGroups.size)}<//>
        <span class="spacer"></span>
        <${MenuButton} class="btn small" disabled=${checkedGroups.size === 0} items=${() => sectionMenu(checkedGroups, undefined).submenu}>${t("섹션")}<${Icon} name="caret-down-bold" size=${9} /><//>
        <button type="button" class="btn small" title=${t("고른 그룹을 보관함으로 옮기기")} disabled=${checkedGroups.size === 0} onClick=${() => archiveGroups(checkedGroups)}>${t("보관")}</button>
        <button type="button" class="btn small destructive" disabled=${checkedGroups.size === 0} onClick=${() => setPendingDelete({ kind: "groups", ids: checkedGroups })}>${t("삭제")}</button>
      </div>`}
    <div class="divider"></div>
    <button
      type="button"
      class=${classes("archive-button", showingArchive && "active", archiveDropping && "drop-target")}
      title=${t("보관한 그룹 · 페이지 보기 · 복원 (그룹 · 섹션 · 페이지를 끌어다 놓으면 보관)")}
      onClick=${() => {
        select(null);
        setShowingArchive(true);
      }}
      onDragOver=${(event) => {
        if (draggingGroups() || draggingSection() || dragging?.kind === "links") {
          event.preventDefault();
          event.dataTransfer.dropEffect = "move";
          setArchiveDropping(true);
        }
      }}
      onDragLeave=${() => setArchiveDropping(false)}
      onDrop=${(event) => {
        event.preventDefault();
        if (draggingGroups()) {
          archiveGroups(new Set(dragging.ids));
          showNotice(t("그룹 {0}개를 보관함으로 옮겼습니다", dragging.ids.length));
        } else if (draggingSection()) {
          const section = sectionByID(dragging.id);
          invoke("archive_section", { id: dragging.id });
          showNotice(t("‘{0}’ 섹션의 그룹을 보관하고 섹션을 지웠습니다", section?.name ?? ""));
        } else if (dragging?.kind === "links") {
          const group = groupByID(dragging.groupID);
          if (group) {
            archiveLinks(group, new Set(dragging.ids));
            showNotice(t("페이지 {0}개를 보관함으로 옮겼습니다", dragging.ids.length));
          }
        }
        clearDrag();
      }}
    >
      <${Icon} name="archive-fill" size=${17} />
      <span>${t("보관함")}</span>
      <span class="spacer"></span>
      ${store.archive.groups.length + store.archive.links.length > 0 &&
      html`<span class="count">${store.archive.groups.length + store.archive.links.length}</span>`}
    </button>
  </aside>`;

  // MARK: 그룹 화면

  const detail = () => {
    if (!selectedGroup) {
      if (showingArchive) {
        return html`<${ArchiveView} store=${store} onShowGroup=${select} />`;
      }
      return html`<${Empty} icon="sidebar-simple" title=${t("그룹을 선택하세요")} />`;
    }
    const group = selectedGroup;
    const chosen = editingLinks ? checkedLinks : linkSelection;
    const rowOf = (id) => document.querySelector(`[data-link="${id}"]`);

    const clickLink = (event, link) => {
      if (editingLinks) {
        setCheckedLinks(toggled(checkedLinks, link.id));
        return;
      }
      if (event.shiftKey && anchorLink) {
        // 시작 줄부터 이 줄까지 이어서 고른다.
        const ids = shownLinks.map((item) => item.id);
        const [from, to] = [ids.indexOf(anchorLink), ids.indexOf(link.id)].sort((a, b) => a - b);
        setLinkSelection(new Set(ids.slice(Math.max(0, from), to + 1)));
        return;
      }
      setAnchorLink(link.id);
      setLinkSelection(event.metaKey || event.ctrlKey ? toggled(linkSelection, link.id) : new Set([link.id]));
    };

    const linkRow = (link, index) => {
      const previewing = previewLinkID === link.id;
      return html`<div
        key=${link.id}
        data-link=${link.id}
        class=${classes(
          "link-row",
          link.opensWithGroup === false && "link-off",
          chosen.has(link.id) && "selected",
          previewing && "previewing",
          linkInsert === index && "insert-before",
          linkInsert === index + 1 && index === shownLinks.length - 1 && "insert-after",
        )}
        title=${link.url}
        draggable=${true}
        onDragStart=${(event) => {
          // 선택(체크)된 줄을 끌면 선택된 페이지 전부를 옮긴다.
          const ids = chosen.has(link.id) ? group.links.map((item) => item.id).filter((id) => chosen.has(id)) : [link.id];
          dragging = { kind: "links", groupID: group.id, ids };
          event.dataTransfer.effectAllowed = "move";
          event.dataTransfer.setData("text/plain", `${boot.dragPrefix}${group.id}|${ids.join(",")}`);
        }}
        onDragEnd=${() => {
          dragging = null;
          setLinkInsert(null);
          setDropTargetGroup(null);
        }}
        onDragOver=${(event) => {
          if (dragging?.kind === "links" && dragging.groupID === group.id && !filteringLinks) {
            event.preventDefault();
            event.dataTransfer.dropEffect = "move";
            setLinkInsert(insertionIndex(event, index));
          }
        }}
        onDrop=${(event) => {
          if (dragging?.kind === "links" && dragging.groupID === group.id && !filteringLinks) {
            event.preventDefault();
            const order = group.links.map((item) => item.id);
            invoke("move_links_within", {
              groupId: group.id,
              from: dragging.ids.map((id) => order.indexOf(id)).filter((position) => position >= 0),
              to: insertionIndex(event, index),
            });
          }
          dragging = null;
          setLinkInsert(null);
        }}
        onClick=${(event) => clickLink(event, link)}
        onDblClick=${() => {
          if (!editingLinks) {
            closePreview();
            openLinks(group, new Set([link.id]));
          }
        }}
        onContextMenu=${(event) => {
          event.stopPropagation();
          // 고르지 않은 줄에서 메뉴를 열면 그 줄만 고른다.
          const ids = chosen.has(link.id) ? chosen : new Set([link.id]);
          if (!chosen.has(link.id) && !editingLinks) {
            setLinkSelection(ids);
            setAnchorLink(link.id);
          }
          showMenu(event, linkMenu(group, ids, rowOf));
        }}
      >
        ${editingLinks && html`<${Check} state=${checkedLinks.has(link.id) ? "on" : "off"} onToggle=${() => setCheckedLinks(toggled(checkedLinks, link.id))} />`}
        ${/* 그룹을 열 때 이 페이지도 열지 (원작처럼 줄 맨 왼쪽) */ ""}
        ${!editingLinks &&
        html`<${MiniSwitch}
          on=${link.opensWithGroup !== false}
          title=${link.opensWithGroup === false ? t("그룹을 열 때 이 페이지는 열지 않습니다 (누르면 켜기)") : t("그룹을 열 때 이 페이지도 엽니다 (누르면 끄기)")}
          onChange=${(opens) => invoke("set_link_opens_with_group", { groupId: group.id, linkId: link.id, opens })}
        />`}
        <${Favicon} url=${link.url} size=${16} />
        <span class="texts">
          <span class="truncate"><${Highlight} text=${linkName(link)} terms=${linkTerms?.text} /></span>
          ${link.alias && html`<span class="url truncate"><${Highlight} text=${link.url} terms=${linkTerms?.text} /></span>`}
        </span>
        ${!editingLinks &&
        html`<span class="row-buttons" onClick=${(event) => event.stopPropagation()} onDblClick=${(event) => event.stopPropagation()}>
          <button type="button" class="btn plain" title=${t("미리보기")} onClick=${(event) => togglePreview(group, link, event.currentTarget.closest(".link-row"))}>
            <${Icon} name=${previewing ? "eye-fill" : "eye"} size=${16} />
          </button>
          <button type="button" class="btn plain" title=${t("편집")} onClick=${() => setLinkSheet({ kind: "edit", groupID: group.id, link })}>
            <${Icon} name="pencil-simple" size=${16} />
          </button>
          <button type="button" class="btn plain" title=${t("시크릿 모드로 열기 (Edge 는 InPrivate)")} onClick=${() => openLinks(group, new Set([link.id]), true)}>
            <${IncognitoIcon} size=${16} />
          </button>
          ${/* 잘못 누르지 않도록 다른 버튼과 떨어뜨린다. */ ""}
          <span class="vdivider"></span>
          <button
            type="button"
            class="btn plain danger-text"
            title=${t("삭제 (확인 없이 바로 지웁니다)")}
            onClick=${() => {
              if (previewing) {
                closePreview();
              }
              setLinkSelection(new Set([...linkSelection].filter((id) => id !== link.id)));
              invoke("delete_links", { groupId: group.id, ids: [link.id] });
            }}
          >
            <${Icon} name="trash" size=${16} />
          </button>
        </span>`}
      </div>`;
    };

    // 페이지 목록: ↑↓ 페이지, ← 그룹 목록으로, Return 열기, Tab 메뉴
    const linkListKey = (event) => {
      if (isComposingKey(event)) {
        return;
      }
      const order = shownLinks.map((link) => link.id);
      const current = order.findIndex((id) => id === anchorLink && linkSelection.has(id));
      if ((event.key === "ArrowDown" || event.key === "ArrowUp") && !editingLinks && order.length > 0) {
        event.preventDefault();
        const step = event.key === "ArrowDown" ? 1 : -1;
        const next = order[current < 0 ? (step > 0 ? 0 : order.length - 1) : Math.max(0, Math.min(order.length - 1, current + step))];
        setLinkSelection(new Set([next]));
        setAnchorLink(next);
        rowOf(next)?.scrollIntoView({ block: "nearest" });
      } else if (event.key === "ArrowLeft" && !editingLinks) {
        event.preventDefault();
        focusGroupList();
      } else if (event.key === "Tab" && !editingLinks) {
        event.preventDefault();
        const ids = linkSelection.size > 0 ? linkSelection : new Set(order.slice(0, 1));
        const anchor = rowOf(anchorLink && ids.has(anchorLink) ? anchorLink : [...ids][0]);
        if (ids.size > 0 && anchor) {
          if (linkSelection.size === 0) {
            setLinkSelection(ids);
            setAnchorLink([...ids][0]);
          }
          showMenu(anchor, linkMenu(group, ids, rowOf));
        }
      } else if (event.key === "Enter" && !editingLinks && linkSelection.size > 0) {
        closePreview();
        openLinks(group, linkSelection);
      } else if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "a") {
        event.preventDefault();
        const all = new Set(shownLinks.map((link) => link.id));
        if (editingLinks) {
          setCheckedLinks(all);
        } else {
          setLinkSelection(all);
        }
      } else if (event.key === "Escape" && editingLinks) {
        setEditingLinks(false);
        setCheckedLinks(new Set());
      }
    };

    const browserName = browserInfo(group.browser).name;
    return html`
      <div class="group-header">
        <div class="title">
          <div class="row" style="gap:10px">
            <${GroupIcon} icon=${group.icon} color=${group.color} size=${34} />
            <h1 class="truncate"><${Highlight} text=${group.name} terms=${terms?.text} /></h1>
            <button type="button" class="btn plain" title=${t("그룹 편집 (이름 · 색 · 아이콘 · 태그 · 브라우저)")} onClick=${() => setGroupSheet({ original: group })}>
              <${Icon} name="pencil-simple" size=${16} />
            </button>
            <button
              type="button"
              class="btn plain"
              title=${t("동기화 — {0} 에 같은 이름으로 열린 탭 그룹과 비교해 맞춥니다", browserName)}
              onClick=${() => setSyncingGroup(group.id)}
            >
              <${Icon} name="arrows-clockwise" size=${16} />
            </button>
          </div>
          <${TagChips} tags=${group.tags} terms=${terms} />
        </div>
        <div class="buttons">
          ${group.links.length > 0 &&
          html`<div class="group-search"><${SearchField} value=${groupQuery} onChange=${setGroupQuery} placeholder=${t("이 그룹에서 찾기")} /></div>`}
          <button type="button" class="btn" title=${t("Chrome · Edge · Whale 에 열린 탭을 골라 이 그룹에 추가")} onClick=${() => setLinkSheet({ kind: "importTabs", groupID: group.id })}>
            <${Icon} name="tray-arrow-down" size=${15} />${t("열린 탭 가져오기")}
          </button>
          <button type="button" class="btn" onClick=${() => setLinkSheet({ kind: "new", groupID: group.id })}>
            <${Icon} name="plus-bold" size=${13} />${t("페이지 추가")}
          </button>
          <${MenuButton}
            title=${t("브라우저 (현재 {0}) — 눌러서 바꾸기", browserName)}
            items=${() => [
              ...browserMenuItems(group.browser, (browser) => invoke("set_group_browser", { id: group.id, browser })),
              "-",
              {
                label: t("항상 시크릿 모드로 열기"),
                checked: group.alwaysPrivate,
                action: () => invoke("set_group_always_private", { id: group.id, alwaysPrivate: !group.alwaysPrivate }),
              },
            ]}
          >
            <${BrowserIcon} browser=${group.browser} size=${16} />
            ${group.alwaysPrivate ? t("{0} 시크릿", browserName) : browserName}
            <${Icon} name="caret-down-bold" size=${9} />
          <//>
          <button
            type="button"
            class="btn primary"
            disabled=${group.links.length === 0}
            title=${group.links.some((link) => link.opensWithGroup !== false) ? t("켜 둔 페이지를 모두 엽니다") : t("페이지가 모두 꺼져 있습니다")}
            onClick=${() => openGroup(group)}
          >
            <${Icon} name="arrow-square-out" size=${15} />${t("그룹 열기")}
          </button>
        </div>
      </div>
      <div class="divider"></div>
      <div class="link-bar">
        ${editingLinks
          ? html`
              <${SelectAll}
                selected=${checkedLinks.size}
                selectable=${shownLinks.length}
                onSelectAll=${() => setCheckedLinks(new Set(shownLinks.map((link) => link.id)))}
                onDeselectAll=${() => setCheckedLinks(new Set())}
              >${t("{0}개 선택", checkedLinks.size)}<//>
              <span class="spacer"></span>
              <${MenuButton}
                class="btn small"
                disabled=${checkedLinks.size === 0 || store.groups.length < 2}
                items=${() => moveMenu(group, checkedLinks).submenu}
              >${t("다른 그룹으로 이동")}<${Icon} name="caret-down-bold" size=${9} /><//>
              <button type="button" class="btn small" title=${t("고른 페이지를 보관함으로 옮기기")} disabled=${checkedLinks.size === 0} onClick=${() => archiveLinks(group, checkedLinks)}>${t("보관")}</button>
              <button
                type="button"
                class="btn small destructive"
                disabled=${checkedLinks.size === 0}
                onClick=${() => setPendingDelete({ kind: "links", groupID: group.id, ids: checkedLinks })}
              >${t("삭제")}</button>
              <button
                type="button"
                class="btn small"
                onClick=${() => {
                  setEditingLinks(false);
                  setCheckedLinks(new Set());
                }}
              >${t("완료")}</button>
            `
          : html`
              <span class="caption truncate">
                ${filteringLinks && shownLinks.length !== group.links.length
                  ? t("검색어에 맞는 페이지 {0}개 / 전체 {1}개", shownLinks.length, group.links.length)
                  : group.links.length > 0
                    ? t("더블클릭: 열기 · 끌기: 순서 변경 / 왼쪽 그룹에 놓아 이동 · 스위치를 끄면 그룹을 열 때 그 페이지는 열지 않음")
                    : ""}
              </span>
              <span class="spacer"></span>
              <button
                type="button"
                class="btn small"
                title=${t("체크박스로 골라 한 번에 삭제 · 이동")}
                disabled=${group.links.length === 0}
                onClick=${() => {
                  setEditingLinks(true);
                  setCheckedLinks(new Set());
                  setLinkSelection(new Set());
                  closePreview();
                }}
              >
                <${Icon} name="list-checks" size=${14} />${t("목록 편집")}
              </button>
            `}
      </div>
      <div class="link-list" tabindex="0" onKeyDown=${linkListKey} onClick=${(event) => {
        // 빈 곳을 누르면 선택을 푼다.
        if (event.target === event.currentTarget && !editingLinks) {
          setLinkSelection(new Set());
        }
      }}>
        ${group.links.length === 0
          ? html`<${Empty} icon="link" title=${t("페이지가 없습니다")}>
              <button type="button" class="btn" onClick=${() => setLinkSheet({ kind: "new", groupID: group.id })}>${t("페이지 추가")}</button>
            <//>`
          : shownLinks.map(linkRow)}
      </div>
    `;
  };

  const sheetGroup = linkSheet ? groupByID(linkSheet.groupID) : null;
  return html`
    ${/* 숨겨도 그려 두고 접는다 (밀려 들어가고 나오는 애니메이션). 접혀 있으면 키보드 · 마우스가 닿지 않게 한다. */ ""}
    <div class=${classes("sidebar-frame", sidebarHidden && "collapsed")} inert=${sidebarHidden}>${sidebar}</div>
    <main class=${classes("detail", sidebarHidden && "sidebar-hidden")}>
      ${boot.platform === "macos"
        ? html`<div class="window-title" data-tauri-drag-region>
            ${sidebarHidden &&
            html`<button type="button" class="btn plain" title=${t("목록 보기")} onClick=${toggleSidebar}><${Icon} name="sidebar-simple" size=${17} /></button>`}
            <span data-tauri-drag-region>LinkGroupz</span>
          </div>`
        : sidebarHidden &&
          html`<button type="button" class="btn plain show-sidebar" title=${t("목록 보기")} onClick=${toggleSidebar}><${Icon} name="sidebar-simple" size=${17} /></button>`}
      ${detail()}
      <${UpdateBadge} />
    </main>

    ${groupSheet &&
    html`<${GroupEditSheet}
      original=${groupSheet.original}
      knownTags=${store.allTags}
      sections=${store.sections}
      section=${groupSheet.section}
      onClose=${() => setGroupSheet(null)}
      onSave=${async (input) => {
        if (groupSheet.original) {
          await invoke("update_group", { id: groupSheet.original.id, input });
        } else {
          select(await invoke("add_group", { input }));
        }
      }}
    />`}
    ${linkSheet?.kind === "new" &&
    html`<${LinkEditSheet} onClose=${() => setLinkSheet(null)} onSave=${(url, alias) => invoke("add_link", { groupId: linkSheet.groupID, url, alias })} />`}
    ${linkSheet?.kind === "edit" &&
    html`<${LinkEditSheet}
      original=${linkSheet.link}
      onClose=${() => setLinkSheet(null)}
      onSave=${(url, alias) => invoke("update_link", { groupId: linkSheet.groupID, linkId: linkSheet.link.id, url, alias })}
    />`}
    ${linkSheet?.kind === "importTabs" && sheetGroup && html`<${ImportTabsSheet} group=${sheetGroup} onClose=${() => setLinkSheet(null)} />`}
    ${importingTabGroups &&
    html`<${ImportTabGroupsSheet}
      onClose=${() => setImportingTabGroups(false)}
      onImported=${(first) => {
        if (first) {
          select(first);
        }
      }}
    />`}
    ${sectionSheet?.kind === "new" &&
    html`<${SectionNameSheet}
      title=${t("새 섹션")}
      saveTitle=${t("만들기")}
      description=${sectionSheet.groupIDs.length > 0 ? t("그룹 {0}개를 담은 섹션을 그 자리에 만듭니다.", sectionSheet.groupIDs.length) : t("그룹을 끌어 넣거나 오른쪽 클릭 → ‘그룹 추가’로 넣습니다.")}
      onClose=${() => setSectionSheet(null)}
      onSave=${(name) => invoke("add_section", { name, at: sectionSheet.at ?? null, groupIds: sectionSheet.groupIDs })}
    />`}
    ${sectionSheet?.kind === "rename" &&
    html`<${SectionNameSheet}
      title=${t("섹션 편집")}
      initial=${sectionSheet.section.name}
      onClose=${() => setSectionSheet(null)}
      onSave=${(name) => invoke("rename_section", { id: sectionSheet.section.id, name })}
    />`}
    ${syncingGroup &&
    groupByID(syncingGroup) &&
    html`<${GroupSyncSheet} group=${groupByID(syncingGroup)} onClose=${() => setSyncingGroup(null)} onDone=${showNotice} />`}
    ${pendingDelete &&
    html`<${Confirm} title=${deleteTitle} message=${deleteMessage} confirmLabel=${t("삭제")} onConfirm=${performDelete} onCancel=${() => setPendingDelete(null)} />`}

    <div class="toast">
      ${notice && html`<div class="notice">${notice}</div>`}
      ${store.lastError && html`<div class="error">${store.lastError}</div>`}
    </div>
  `;
}

render(html`<${Manager} />`, document.getElementById("app"));
afterPaint(() => document.documentElement.classList.add("ready"));
