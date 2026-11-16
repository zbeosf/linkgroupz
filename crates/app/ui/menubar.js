// 메뉴바(트레이) 아이콘을 눌렀을 때 뜨는 창: 그룹을 검색하고 바로 연다.

import {
  boot,
  browserInfo,
  browserMenuItems,
  Chevron,
  classes,
  colorCss,
  Favicon,
  GroupCardContent,
  Highlight,
  html,
  Icon,
  invoke,
  isComposingKey,
  linkName,
  listen,
  MenuButton,
  ProfileSwitcher,
  render,
  SearchField,
  SearchScopeButton,
  searchTerms,
  showMenu,
  sidebarItems,
  t,
  toggled,
  useEffect,
  useFiltered,
  useProfiles,
  useRef,
  useState,
  useStore,
  useUndoKeys,
} from "./lib/ui.js";

const initialLayout = await invoke("popover_layout");

/** 끌고 있는 것 (끄는 동안에는 dataTransfer 를 읽을 수 없어 따로 기억한다) */
let dragging = null; // { kind: "groups", ids } | { kind: "section", id } | { kind: "links", groupID, ids }

/** 줄 안에서 마우스가 위에서부터 어디쯤인지 (0 ~ 1) */
function verticalFraction(event) {
  const box = event.currentTarget.getBoundingClientRect();
  return (event.clientY - box.top) / Math.max(1, box.height);
}

function MenuBar() {
  const store = useStore();
  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState(
    // 사용법 스크린샷용: 첫 그룹을 펼쳐 둔다
    () => new Set(boot.scene === "menubar" && store.groups[0] ? [store.groups[0].id] : []),
  );
  const [shownCount, setShownCount] = useState(0);
  // 사용자가 창 가장자리를 끌어 크기를 정해 두었는지 (아니면 높이를 내용에 맞춘다)
  const [fixed, setFixed] = useState(initialLayout.fixed);
  // 메뉴바에서 떼어 놓았는지 (끌어서 옮기고, 다른 곳을 눌러도 닫히지 않는다)
  const [detached, setDetached] = useState(initialLayout.detached);
  const root = useRef(null);
  // 메뉴(오른쪽 클릭 · Tab · 프로필 · 📌)가 창 아래로 넘치면 그만큼 창 아래에 빈 자리를 둔다 (창이 내용에 맞춰 커진다).
  const [menuRoom, setMenuRoom] = useState(0);
  // 되돌리기 알림 (⌘Z)
  const [notice, setNotice] = useState(null);
  useUndoKeys((message) => {
    setNotice(message);
    setTimeout(() => setNotice((current) => (current === message ? null : current)), 3000);
  });
  useEffect(() => {
    const onRoom = (event) => {
      if (!event.detail) {
        setMenuRoom(0);
        return;
      }
      setMenuRoom((current) => {
        const base = (root.current?.getBoundingClientRect().height ?? 0) - current;
        return Math.max(current, Math.ceil(event.detail - base));
      });
    };
    window.addEventListener("menu-room", onRoom);
    return () => window.removeEventListener("menu-room", onRoom);
  }, []);

  const filtering = query.trim() !== "";
  const visibleGroups = useFiltered("filter_groups", query, store, store.groups);
  const terms = filtering ? searchTerms(query) : null;
  const items = sidebarItems(store, filtering ? visibleGroups : null);

  // 창이 뜰 때마다 검색창에 커서를 둔다.
  useEffect(
    () =>
      listen("popover-shown", (layout) => {
        setShownCount((count) => count + 1);
        setFixed(Boolean(layout?.fixed));
        setDetached(Boolean(layout?.detached));
      }),
    [],
  );
  // 창 가장자리를 끌어 크기를 바꾸거나 떼어 놓으면 앱이 알려 준다.
  useEffect(
    () =>
      listen("popover-layout", (layout) => {
        setFixed(Boolean(layout?.fixed));
        setDetached(Boolean(layout?.detached));
      }),
    [],
  );
  useEffect(() => {
    document.documentElement.classList.toggle("detached", detached);
  }, [detached]);

  const windowMenu = () => [
    detached
      ? { label: t("메뉴바에 붙이기"), action: () => invoke("set_popover_detached", { detached: false }) }
      : { label: t("메뉴바에서 떼어 놓기"), action: () => invoke("set_popover_detached", { detached: true }) },
    {
      label: t("창 크기 원래대로"),
      disabled: !fixed,
      action: () => {
        invoke("set_popover_size", { size: null, done: true });
        setFixed(false);
      },
    },
  ];

  // 크기를 정해 뒀으면 창을 꽉 채우고 목록이 그 안에서 스크롤된다.
  useEffect(() => {
    document.documentElement.classList.toggle("fixed-size", fixed);
  }, [fixed]);

  // 내용 높이가 바뀌면 창 높이를 맞춘다.
  useEffect(() => {
    const element = root.current;
    let last = 0;
    const report = () => {
      const height = Math.ceil(element.getBoundingClientRect().height);
      if (height !== last) {
        last = height;
        invoke("resize_popover", { height });
      }
    };
    const observer = new ResizeObserver(report);
    observer.observe(element);
    report();
    return () => observer.disconnect();
  }, []);

  // 메뉴바에 붙어 있으면 열고 나서 닫는다. 떼어 놓은 창은 그대로 둔다.
  const hideAfter = (work) => {
    work();
    if (!detached) {
      invoke("hide_popover");
    }
  };

  // MARK: 키보드로 고르기 — ↑↓ 로 섹션 · 그룹 · 페이지를 고르고 Enter 로 연다. → 펼치기, ← 접기.
  const fullGroup = (shown) => store.groups.find((group) => group.id === shown.id) ?? shown;
  const navItems = [];
  const pushGroup = (shown) => {
    navItems.push({ kind: "group", key: `g:${shown.id}`, group: fullGroup(shown) });
    if (filtering || expanded.has(shown.id)) {
      for (const link of shown.links) {
        navItems.push({ kind: "link", key: `l:${shown.id}:${link.id}`, group: fullGroup(shown), link });
      }
    }
  };
  if (visibleGroups.length > 0) {
    for (const item of items) {
      if (item.kind === "group") {
        pushGroup(item.group);
        continue;
      }
      navItems.push({ kind: "section", key: `s:${item.section.id}`, section: item.section });
      if (filtering || !item.section.isCollapsed) {
        item.groups.forEach(pushGroup);
      }
    }
  }
  const [cursor, setCursor] = useState(null);
  // 검색하면 맨 위 결과를 골라 둔다 (입력하고 바로 Enter 로 연다). 창을 다시 띄우면 처음부터.
  useEffect(() => {
    setCursor(null);
  }, [shownCount]);
  // 프로필을 바꾸면 펼침 · 고른 줄을 처음부터 (처음 그릴 때는 그대로 둔다).
  const profileID = useProfiles().current;
  const shownProfile = useRef(profileID);
  useEffect(() => {
    if (shownProfile.current !== profileID) {
      shownProfile.current = profileID;
      setExpanded(new Set());
      setCursor(null);
    }
  }, [profileID]);
  const firstOpenable = navItems.find((item) => item.kind !== "section")?.key ?? null;
  const current = navItems.find((item) => item.key === cursor) ?? (filtering ? navItems.find((item) => item.key === firstOpenable) : null);

  const openItem = (item) => {
    if (item.kind === "group") {
      hideAfter(() => invoke("open_group", { id: item.group.id }));
    } else if (item.kind === "link") {
      hideAfter(() => invoke("open_links", { groupId: item.group.id, ids: [item.link.id], private: false }));
    } else if (!filtering) {
      invoke("set_section_collapsed", { id: item.section.id, collapsed: !item.section.isCollapsed });
    }
  };

  useEffect(() => {
    const onKey = (event) => {
      if (event.key === "Escape") {
        invoke("hide_popover");
        return;
      }
      // 메뉴(프로필 · 📌)가 떠 있으면 메뉴가 키를 받는다.
      if (isComposingKey(event) || event.metaKey || event.ctrlKey || event.altKey || document.querySelector(".menu-layer")) {
        return;
      }
      const index = current ? navItems.indexOf(current) : -1;
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        if (navItems.length === 0) {
          return;
        }
        const step = event.key === "ArrowDown" ? 1 : -1;
        const next = index < 0 ? (step > 0 ? 0 : navItems.length - 1) : Math.max(0, Math.min(navItems.length - 1, index + step));
        setCursor(navItems[next].key);
      } else if (event.key === "Enter" && current) {
        event.preventDefault();
        openItem(current);
      } else if (event.key === "Tab" && current) {
        // Tab: 고른 줄의 오른쪽 클릭 메뉴
        event.preventDefault();
        const row = document.querySelector(`[data-nav="${CSS.escape(current.key)}"]`);
        showMenu(row?.querySelector(".group-card") ?? row, menuFor(current));
      } else if ((event.key === "ArrowRight" || event.key === "ArrowLeft") && current && !query) {
        // 검색어를 입력하는 중에는 ←→ 를 글자 사이 이동에 둔다.
        event.preventDefault();
        const open = event.key === "ArrowRight";
        if (current.kind === "group") {
          setExpanded((previous) => {
            const next = new Set(previous);
            if (open) {
              next.add(current.group.id);
            } else {
              next.delete(current.group.id);
            }
            return next;
          });
        } else if (current.kind === "section") {
          invoke("set_section_collapsed", { id: current.section.id, collapsed: !open });
        } else if (!open) {
          // 페이지에서 ← 는 그 그룹으로 올라간다.
          setCursor(`g:${current.group.id}`);
        }
      }
    };
    // 검색창이 먼저 받게 둔다 (검색어가 있으면 Esc 는 검색어만 지운다).
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // 고른 줄이 보이게 스크롤한다.
  useEffect(() => {
    if (current) {
      document.querySelector(`[data-nav="${CSS.escape(current.key)}"]`)?.scrollIntoView({ block: "nearest" });
    }
  }, [current?.key]);
  const active = (key) => current?.key === key;

  // MARK: 오른쪽 클릭 메뉴 — 관리 창과 같은 메뉴. 편집 · 페이지 추가 · 미리보기 · 삭제처럼 입력이나 확인이 필요한 항목은 관리 창을 열어 이어서 한다.
  const inManager = (action) => invoke("request_manager_action", { action });
  const moveToSection = (ids, sectionID) => invoke("move_items", { ids, target: { section: sectionID } });

  const groupMenu = (group) => {
    const empty = group.links.length === 0;
    return [
      { label: t("그룹 열기"), disabled: empty, action: () => hideAfter(() => invoke("open_group", { id: group.id })) },
      { label: t("새 창으로 열기"), disabled: empty, action: () => hideAfter(() => invoke("open_group", { id: group.id, newWindow: true })) },
      { label: t("시크릿 모드로 열기"), disabled: empty, action: () => hideAfter(() => invoke("open_group_private", { id: group.id })) },
      "-",
      { label: t("브라우저"), submenu: browserMenuItems(group.browser, (browser) => invoke("set_group_browser", { id: group.id, browser })) },
      {
        label: t("항상 시크릿 모드로 열기"),
        checked: group.alwaysPrivate,
        action: () => invoke("set_group_always_private", { id: group.id, alwaysPrivate: !group.alwaysPrivate }),
      },
      {
        label: t("섹션"),
        submenu: [
          ...store.sections.map((section) => ({ label: section.name, checked: group.sectionID === section.id, action: () => moveToSection([group.id], section.id) })),
          ...(store.sections.length > 0 ? ["-"] : []),
          { label: t("섹션 없음"), checked: !group.sectionID, action: () => moveToSection([group.id], null) },
        ],
      },
      "-",
      { label: t("편집…"), action: () => inManager({ kind: "editGroup", id: group.id }) },
      { label: t("페이지 추가…"), action: () => inManager({ kind: "addLink", groupId: group.id }) },
      { label: t("열린 탭 가져오기…"), action: () => inManager({ kind: "importTabs", groupId: group.id }) },
      { label: t("브라우저 탭 그룹과 동기화…"), action: () => inManager({ kind: "syncGroup", id: group.id }) },
      "-",
      { label: t("보관함으로 이동"), action: () => invoke("archive_groups", { ids: [group.id] }) },
      { label: t("삭제…"), destructive: true, action: () => inManager({ kind: "deleteGroup", id: group.id }) },
    ];
  };

  const linkMenu = (group, link) => {
    const ids = [link.id];
    const targets = store.groups.filter((other) => other.id !== group.id);
    return [
      { label: t("열기"), action: () => hideAfter(() => invoke("open_links", { groupId: group.id, ids, private: false })) },
      { label: t("새 창으로 열기"), action: () => hideAfter(() => invoke("open_links", { groupId: group.id, ids, private: false, newWindow: true })) },
      { label: t("시크릿 모드로 열기"), action: () => hideAfter(() => invoke("open_links", { groupId: group.id, ids, private: true })) },
      {
        label: t("그룹을 열 때 함께 열기"),
        checked: link.opensWithGroup !== false,
        action: () => invoke("set_link_opens_with_group", { groupId: group.id, linkId: link.id, opens: link.opensWithGroup === false }),
      },
      { label: t("URL 복사"), action: () => invoke("copy_text", { text: link.url }) },
      {
        label: t("다른 그룹으로 이동"),
        disabled: targets.length === 0,
        submenu: targets.map((target) => ({ label: target.name, action: () => invoke("move_links", { ids, sourceId: group.id, targetId: target.id }) })),
      },
      "-",
      { label: t("편집…"), action: () => inManager({ kind: "editLink", groupId: group.id, linkId: link.id }) },
      { label: t("미리보기…"), action: () => inManager({ kind: "previewLink", groupId: group.id, linkId: link.id }) },
      "-",
      { label: t("보관함으로 이동"), action: () => invoke("archive_links", { groupId: group.id, ids }) },
      { label: t("삭제…"), destructive: true, action: () => inManager({ kind: "deleteLinks", groupId: group.id, ids }) },
    ];
  };

  const sectionMenu = (section) => {
    const order = store.sidebarOrder ?? [];
    const index = order.indexOf(`section:${section.id}`);
    const outside = store.groups.filter((group) => group.sectionID !== section.id);
    return [
      { label: section.isCollapsed ? t("펼치기") : t("접기"), disabled: filtering, action: () => invoke("set_section_collapsed", { id: section.id, collapsed: !section.isCollapsed }) },
      {
        label: t("그룹 추가"),
        disabled: outside.length === 0,
        submenu: outside.map((group) => ({ label: group.name, action: () => moveToSection([group.id], section.id) })),
      },
      { label: t("위로 이동"), disabled: index <= 0, action: () => invoke("move_section", { id: section.id, offset: -1 }) },
      { label: t("아래로 이동"), disabled: index < 0 || index >= order.length - 1, action: () => invoke("move_section", { id: section.id, offset: 1 }) },
      "-",
      { label: t("새 그룹 만들기…"), action: () => inManager({ kind: "newGroupInSection", sectionId: section.id }) },
      { label: t("섹션 편집…"), action: () => inManager({ kind: "editSection", sectionId: section.id }) },
      "-",
      { label: t("섹션 삭제…"), destructive: true, action: () => inManager({ kind: "deleteSection", sectionId: section.id }) },
    ];
  };

  // MARK: 끌어서 옮기기 — 그룹 · 섹션 순서, 섹션에 넣고 빼기, 페이지 순서, 페이지를 다른 그룹으로

  const [drop, setDrop] = useState(null); // { key, position: "before" | "after" | "onto" }
  const clearDrag = () => {
    dragging = null;
    setDrop(null);
  };
  const dropMark = (key) => (drop?.key === key ? drop.position : null);
  const markClass = (key) => {
    const position = dropMark(key);
    return position === "before" ? "insert-before" : position === "after" ? "insert-after" : position === "onto" ? "drop-target" : null;
  };

  /** 이 그룹 뒤 자리: 섹션 안이면 다음 그룹 앞(없으면 섹션 끝), 맨 위 단계면 이 그룹 바로 뒤 */
  const afterGroup = (group, sectionID) => {
    if (!sectionID) {
      return { section: null, after: group.id };
    }
    const inSection = store.groups.filter((other) => other.sectionID === sectionID);
    const next = inSection[inSection.findIndex((other) => other.id === group.id) + 1];
    return next ? { section: sectionID, before: next.id } : { section: sectionID };
  };

  const moveTo = (target) => {
    if (dragging?.kind === "groups") {
      invoke("move_items", { ids: dragging.ids, target });
    } else if (dragging?.kind === "section") {
      invoke("move_items", { ids: [dragging.id], target: { section: null, before: target.before ?? null, after: target.after ?? null } });
    }
    clearDrag();
  };

  /** 그룹 줄: 그룹 · 섹션을 그 앞뒤로, 다른 그룹의 페이지는 이 그룹으로 */
  const groupDrag = (group, sectionID) => {
    const key = `g:${group.id}`;
    const linksHere = () => dragging?.kind === "links" && dragging.groupID !== group.id;
    const itemsHere = () => !filtering && ((dragging?.kind === "groups" && !dragging.ids.includes(group.id)) || (dragging?.kind === "section" && !sectionID));
    return {
      draggable: !filtering,
      onDragStart: (event) => {
        dragging = { kind: "groups", ids: [group.id] };
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", group.name);
      },
      onDragEnd: clearDrag,
      onDragOver: (event) => {
        if (linksHere()) {
          event.preventDefault();
          setDrop({ key, position: "onto" });
        } else if (itemsHere()) {
          event.preventDefault();
          setDrop({ key, position: verticalFraction(event) < 0.5 ? "before" : "after" });
        }
      },
      onDrop: (event) => {
        event.preventDefault();
        if (linksHere()) {
          invoke("move_links", { ids: dragging.ids, sourceId: dragging.groupID, targetId: group.id });
          clearDrag();
        } else if (itemsHere()) {
          moveTo(verticalFraction(event) < 0.5 ? { section: sectionID ?? null, before: group.id } : afterGroup(group, sectionID));
        } else {
          clearDrag();
        }
      },
    };
  };

  /** 섹션 머리줄: 그룹은 위쪽 끝이면 섹션 앞, 그 밖이면 섹션 안(끝)으로. 섹션은 앞뒤로. */
  const sectionDrag = (section) => {
    const key = `s:${section.id}`;
    const position = (event) => {
      const fraction = verticalFraction(event);
      if (dragging?.kind === "groups") {
        return fraction < 0.25 ? "before" : "onto";
      }
      return fraction < 0.5 ? "before" : "after";
    };
    const droppable = () => !filtering && (dragging?.kind === "groups" || (dragging?.kind === "section" && dragging.id !== section.id));
    return {
      draggable: !filtering,
      onDragStart: (event) => {
        dragging = { kind: "section", id: section.id };
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", section.name);
      },
      onDragEnd: clearDrag,
      onDragOver: (event) => {
        if (droppable()) {
          event.preventDefault();
          setDrop({ key, position: position(event) });
        }
      },
      onDrop: (event) => {
        event.preventDefault();
        if (!droppable()) {
          clearDrag();
          return;
        }
        const where = position(event);
        moveTo(where === "onto" ? { section: section.id } : where === "before" ? { section: null, before: section.id } : { section: null, after: section.id });
      },
    };
  };

  /** 페이지 줄: 같은 그룹 안에서 순서를 바꾼다 */
  const linkDrag = (group, link, index) => {
    const key = `l:${group.id}:${link.id}`;
    const here = () => !filtering && dragging?.kind === "links" && dragging.groupID === group.id;
    const insertion = (event) => (verticalFraction(event) < 0.5 ? index : index + 1);
    return {
      draggable: true,
      onDragStart: (event) => {
        dragging = { kind: "links", groupID: group.id, ids: [link.id] };
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", link.url);
      },
      onDragEnd: clearDrag,
      onDragOver: (event) => {
        if (here()) {
          event.preventDefault();
          setDrop({ key, position: insertion(event) === index ? "before" : "after" });
        }
      },
      onDrop: (event) => {
        event.preventDefault();
        if (here()) {
          const order = group.links.map((item) => item.id);
          invoke("move_links_within", { groupId: group.id, from: dragging.ids.map((id) => order.indexOf(id)).filter((at) => at >= 0), to: insertion(event) });
        }
        clearDrag();
      },
    };
  };

  const menuFor = (item) => (item.kind === "group" ? groupMenu(item.group) : item.kind === "link" ? linkMenu(item.group, item.link) : sectionMenu(item.section));
  /** 오른쪽 클릭: 그 줄로 커서를 옮기고 메뉴를 연다 */
  const contextMenu = (event, item) => {
    event.preventDefault();
    event.stopPropagation();
    setCursor(item.key);
    showMenu(event, menuFor(item));
  };

  const groupSection = (shown, inSection = false) => {
    // 검색으로 걸러진 페이지만 보여주되, 그룹 열기는 전체 페이지를 연다.
    const full = store.groups.find((group) => group.id === shown.id) ?? shown;
    const isExpanded = filtering || expanded.has(shown.id);
    return html`
      <div
        class=${classes("popover-group", inSection && "in-section", markClass(`g:${shown.id}`))}
        key=${shown.id}
        data-nav=${`g:${shown.id}`}
        ...${groupDrag(full, inSection ? full.sectionID : null)}
      >
        ${/* 화살표는 '그룹 열기' 카드 밖에 따로 둔다. 안에 두면 접으려고 누른 클릭이 그룹 열기로 가기도 한다. */ ""}
        <${Chevron} expanded=${isExpanded} disabled=${filtering} onToggle=${() => setExpanded(toggled(expanded, shown.id))} />
        <div
          class=${classes("group-card", "hoverable", active(`g:${shown.id}`) && "keyboard")}
          style=${`--tint:${colorCss(full.color)}`}
          title=${t("그룹의 페이지 {0}개를 {1} 에서 열기", full.links.length, browserInfo(full.browser).name)}
          onClick=${() => hideAfter(() => invoke("open_group", { id: full.id }))}
          onContextMenu=${(event) => contextMenu(event, { kind: "group", key: `g:${shown.id}`, group: full })}
        >
          <${GroupCardContent} group=${full} linkCount=${full.links.length} terms=${terms} />
        </div>
      </div>
      ${isExpanded &&
      shown.links.map(
        (link, index) => html`<div
          class=${classes("popover-link", inSection && "in-section", link.opensWithGroup === false && "link-off", active(`l:${shown.id}:${link.id}`) && "keyboard", markClass(`l:${shown.id}:${link.id}`))}
          key=${link.id}
          ...${linkDrag(full, link, full.links.findIndex((item) => item.id === link.id) >= 0 ? full.links.findIndex((item) => item.id === link.id) : index)}
          data-nav=${`l:${shown.id}:${link.id}`}
          title=${link.url}
          onClick=${() => hideAfter(() => invoke("open_links", { groupId: full.id, ids: [link.id], private: false }))}
          onContextMenu=${(event) => contextMenu(event, { kind: "link", key: `l:${shown.id}:${link.id}`, group: full, link })}
        >
          <${Favicon} url=${link.url} size=${14} />
          <span class="texts">
            <span class="truncate"><${Highlight} text=${linkName(link)} terms=${terms?.text} /></span>
            ${link.alias && html`<span class="url truncate"><${Highlight} text=${link.url} terms=${terms?.text} /></span>`}
          </span>
        </div>`,
      )}
    `;
  };

  return html`<div class="popover" ref=${root}>
    ${/* 떼어 놓았으면 맨 위 띠와 빈 곳을 끌어 창을 옮긴다 */ ""}
    ${detached && html`<div class="popover-grab" data-tauri-drag-region title=${t("끌어서 옮기기")}></div>`}
    <div class="popover-top" data-tauri-drag-region=${detached ? "" : undefined}>
      <${ProfileSwitcher} />
      <${SearchField} value=${query} onChange=${setQuery} placeholder=${t("이름 · 태그 · URL")} autofocus=${true} focusSignal=${shownCount} />
      <${SearchScopeButton} />
      <button type="button" class="btn small icon-only" title=${t("펼친 그룹을 모두 접기")} disabled=${expanded.size === 0 || filtering} onClick=${() => setExpanded(new Set())}>
        <${Icon} name="caret-double-up" size=${13} />
      </button>
    </div>
    <div class="divider"></div>
    ${store.groups.length === 0
      ? html`<div class="popover-empty">
          <${Icon} name="folder-plus" size=${30} />
          <b>${t("아직 그룹이 없습니다")}</b>
          <span>${t("아래 ‘관리’ 에서 새 그룹을 만들거나")}<br />${t("브라우저 탭 그룹을 가져오세요.")}</span>
          <button type="button" class="btn small primary" onClick=${() => invoke("show_manager")}>
            <${Icon} name="squares-four" size=${13} />${t("관리 창 열기")}
          </button>
        </div>`
      : visibleGroups.length === 0
        ? html`<div class="popover-empty">${t("검색 결과가 없습니다")}</div>`
        : html`<div class="popover-list">
            ${items.map((item) => {
              if (item.kind === "group") {
                return groupSection(item.group);
              }
              // 섹션: 관리 창과 같은 순서 · 같은 접힘 상태. 검색 중에는 모두 펼친다.
              const { section, groups } = item;
              const open = filtering || !section.isCollapsed;
              return html`<div class="popover-section" key=${section.id}>
                <div
                  class=${classes("popover-section-header", active(`s:${section.id}`) && "keyboard", markClass(`s:${section.id}`))}
                  data-nav=${`s:${section.id}`}
                  ...${sectionDrag(section)}
                  title=${open ? t("접기") : t("펼치기")}
                  onClick=${() => !filtering && invoke("set_section_collapsed", { id: section.id, collapsed: !section.isCollapsed })}
                  onContextMenu=${(event) => contextMenu(event, { kind: "section", key: `s:${section.id}`, section })}
                >
                  <${Icon} name=${open ? "caret-down-bold" : "caret-right-bold"} size=${10} />
                  <span class="truncate"><${Highlight} text=${section.name} terms=${terms?.text} /></span>
                  <span class="count">${groups.length}</span>
                </div>
                ${open && groups.map((group) => groupSection(group, true))}
              </div>`;
            })}
            <div
              class=${classes("popover-list-end", markClass("end"))}
              onDragOver=${(event) => {
                if (!filtering && (dragging?.kind === "groups" || dragging?.kind === "section")) {
                  event.preventDefault();
                  setDrop({ key: "end", position: "before" });
                }
              }}
              onDrop=${(event) => {
                event.preventDefault();
                moveTo({ section: null });
              }}
            ></div>
          </div>`}
    <div class="divider"></div>
    <div class="popover-footer">
      <button type="button" class="btn small primary" title=${t("그룹 · 페이지 편집")} onClick=${() => invoke("show_manager")}>
        <${Icon} name="squares-four" size=${14} />${t("관리")}
      </button>
      <button type="button" class="btn small" onClick=${() => invoke("show_settings")}><${Icon} name="gear" size=${14} />${t("설정")}</button>
      <button type="button" class="btn small" onClick=${() => invoke("show_welcome")}><${Icon} name="question" size=${14} />${t("안내")}</button>
      <span class="spacer" data-tauri-drag-region=${detached ? "" : undefined}></span>
      <${MenuButton}
        class=${classes("btn small icon-only", detached && "on")}
        title=${detached ? t("떼어 놓은 창 — 메뉴바에 붙이기 · 창 크기") : t("메뉴바에서 떼어 놓기 · 창 크기 (가장자리를 끌어 크기를 바꿀 수 있습니다)")}
        items=${windowMenu}
      ><${Icon} name=${detached ? "push-pin-fill" : "push-pin"} size=${14} /><//>
      <button type="button" class="btn small" title=${t("LinkGroupz 종료")} onClick=${() => invoke("quit")}><${Icon} name="power" size=${14} />${t("종료")}</button>
    </div>
    ${menuRoom > 0 && !fixed && html`<div class="menu-room" style=${`height:${menuRoom}px`}></div>`}
    ${notice && html`<div class="toast"><div class="notice">${notice}</div></div>`}
  </div>`;
}

render(html`<${MenuBar} />`, document.getElementById("app"));
