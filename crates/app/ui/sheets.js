// 그룹 · 페이지 편집 시트

import {
  boot,
  BrowserPicker,
  classes,
  colorCss,
  ColorDot,
  colorInfo,
  GroupIcon,
  html,
  Icon,
  IncognitoIcon,
  invoke,
  Sheet,
  SymbolIcon,
  t,
  TagChips,
  useEffect,
  useRef,
  useState,
} from "./lib/ui.js";

/** 그룹 색 고르기. 위 줄은 Chrome 탭 그룹 색, 아래 줄은 앱 전용 색(Chrome 에서는 가장 가까운 색으로 열림). chromeNote=false 면 Chrome 안내를 뺀다 */
export function ColorPalette({ value, onChange, chromeNote = true }) {
  const selected = colorInfo(value);
  const row = (colors) => html`<div class="palette-row">
    ${colors.map(
      (color) => html`<button
        type="button"
        key=${color.id}
        title=${color.isChrome ? color.name : `${color.name} (Chrome: ${color.chromeName})`}
        onClick=${() => onChange(color.id)}
      >
        <${ColorDot} color=${color.id} size=${20} />
        ${value === color.id && html`<${Icon} name="check-bold" size=${11} />`}
      </button>`,
    )}
  </div>`;
  return html`<div class="column" style="gap:8px">
    ${row(boot.colors.filter((color) => color.isChrome))}
    ${row(boot.colors.filter((color) => !color.isChrome))}
    <div class="caption">
      ${!chromeNote
        ? selected.name
        : selected.isChrome
          ? t("{0} — Chrome 탭 그룹도 같은 색", selected.name)
          : t("{0} — Chrome 탭 그룹은 가장 가까운 {1}으로 열림", selected.name, selected.chromeName)}
    </div>
  </div>`;
}

/** 그룹 아이콘 고르기: 기본 모양(한 줄 8개) + 내 이미지. defaultIcon: '기본으로' 버튼이 되돌릴 아이콘 */
export function IconPicker({ value, onChange, color, defaultIcon = { symbol: boot.defaultSymbol }, defaultLabel = t("기본 폴더로") }) {
  const [error, setError] = useState(null);
  const isDefault = value.symbol === defaultIcon.symbol;
  const chooseImage = async () => {
    try {
      const name = await invoke("choose_group_icon_image");
      if (name) {
        onChange({ image: name });
        setError(null);
      }
    } catch (message) {
      setError(String(message));
    }
  };
  return html`<div class="column" style="gap:8px">
    <div class="row" style="align-items:flex-start;gap:12px">
      <${GroupIcon} icon=${value} color=${color} size=${44} />
      <div class="icon-grid">
        ${boot.presetSymbols.map((symbol) => {
          const selected = value.symbol === symbol;
          return html`<button
            type="button"
            key=${symbol}
            class=${classes(selected && "selected")}
            style=${selected ? `background:${colorCss(color)}` : ""}
            title=${symbol}
            onClick=${() => onChange({ symbol })}
          >
            <${SymbolIcon} symbol=${symbol} size=${16} />
          </button>`;
        })}
      </div>
    </div>
    <div class="row">
      <button type="button" class="btn small" onClick=${chooseImage}><${Icon} name="image" size=${13} />${t("이미지 파일 선택…")}</button>
      <button type="button" class="btn small" disabled=${isDefault} onClick=${() => onChange(defaultIcon)}>${defaultLabel}</button>
    </div>
    ${error
      ? html`<div class="error-text">${error}</div>`
      : value.image && html`<div class="caption">${t("내 이미지 — 가운데를 정사각형으로 잘라 앱 데이터 폴더에 복사해 둡니다")}</div>`}
  </div>`;
}

/**
 * 새 그룹 · 그룹 편집.
 * onSave 는 { name, color, tags, browser, icon, alwaysPrivate, sectionID } 를 받아 저장하고, 실패하면 오류를 던진다.
 * sections 를 주면 넣을 섹션을 고를 수 있다 (section: 새 그룹을 넣을 섹션).
 */
export function GroupEditSheet({ original, knownTags, sections, section, title, saveTitle = t("저장"), onSave, onClose }) {
  const [name, setName] = useState(original?.name ?? "");
  const [sectionID, setSectionID] = useState(original ? (original.sectionID ?? null) : (section ?? null));
  const nameInput = useRef(null);
  const tagsInput = useRef(null);
  const [color, setColor] = useState(original?.color ?? "blue");
  const [tagText, setTagText] = useState(original?.tags?.join(", ") ?? "");
  const [browser, setBrowser] = useState(original?.browser ?? boot.browsers[0].id);
  const [icon, setIcon] = useState(original?.icon ?? { symbol: boot.defaultSymbol });
  const [alwaysPrivate, setAlwaysPrivate] = useState(original?.alwaysPrivate ?? false);
  const [tags, setTags] = useState(original?.tags ?? []);
  const [error, setError] = useState(null);

  // 태그 규칙(공백 · # · 중복 정리)은 앱에 있다.
  useEffect(() => {
    let current = true;
    invoke("parse_tags", { text: tagText }).then((parsed) => {
      if (current) {
        setTags(parsed);
      }
    });
    return () => {
      current = false;
    };
  }, [tagText]);

  const trimmedName = name.trim();
  const used = new Set(tags.map((tag) => tag.toLowerCase()));
  const suggested = (knownTags ?? []).filter((tag) => !used.has(tag.toLowerCase())).slice(0, 8);

  const cancel = () => {
    // 고르다 만 아이콘 이미지를 정리한다.
    invoke("cleanup_group_icons");
    onClose();
  };
  const save = async () => {
    try {
      // 입력 칸에서 바로 읽는다: 한글을 조합하는 중에 저장 버튼을 누르면 마지막 글자가 아직 상태에 들어오지 않았다.
      const finalName = (nameInput.current?.value ?? name).trim();
      const parsed = await invoke("parse_tags", { text: tagsInput.current?.value ?? tagText });
      const input = { name: finalName, color, tags: parsed, browser, icon, alwaysPrivate };
      if (sections) {
        input.sectionID = sectionID;
      }
      await onSave(input);
      onClose();
    } catch (message) {
      setError(String(message));
    }
  };

  return html`<${Sheet} width=${520} onClose=${cancel} onSubmit=${save} canSubmit=${trimmedName !== ""}>
    <h2>${title ?? (original ? t("그룹 편집") : t("새 그룹"))}</h2>
    <div class="form">
      <label for="group-name">${t("이름")}</label>
      <input id="group-name" ref=${nameInput} class="field" type="text" spellcheck=${false} value=${name} onInput=${(event) => setName(event.target.value)} />

      ${sections?.length > 0 &&
      html`<label for="group-section">${t("섹션")}</label>
        <select id="group-section" class="field" value=${sectionID ?? ""} onChange=${(event) => setSectionID(event.target.value || null)}>
          <option value="">${t("섹션 없음")}</option>
          ${sections.map((item) => html`<option key=${item.id} value=${item.id}>${item.name}</option>`)}
        </select>`}

      <span class="label top">${t("색")}</span>
      <${ColorPalette} value=${color} onChange=${setColor} />

      <span class="label top">${t("아이콘")}</span>
      <${IconPicker} value=${icon} onChange=${setIcon} color=${color} />

      <span class="label top">${t("브라우저")}</span>
      <div class="column" style="gap:6px;align-items:flex-start">
        <${BrowserPicker} value=${browser} onChange=${setBrowser} />
        <label class="row" style="gap:5px">
          <input type="checkbox" checked=${alwaysPrivate} onChange=${(event) => setAlwaysPrivate(event.target.checked)} />
          <${IncognitoIcon} size=${15} />
          ${browser === "edge" ? t("항상 InPrivate 창으로 열기") : t("항상 시크릿 모드로 열기")}
        </label>
      </div>

      <label for="group-tags">${t("태그")}</label>
      <input
        id="group-tags"
        ref=${tagsInput}
        class="field"
        type="text"
        spellcheck=${false}
        placeholder=${t("쉼표나 공백으로 구분 (예: 업무, daily)")}
        value=${tagText}
        onInput=${(event) => setTagText(event.target.value)}
      />
      ${tags.length > 0 && html`<span></span><${TagChips} tags=${tags} />`}
      ${suggested.length > 0 &&
      html`<span class="label">${t("기존 태그")}</span>
        <div class="row" style="gap:6px;flex-wrap:wrap">
          ${suggested.map(
            (tag) => html`<button type="button" class="btn link" key=${tag} onClick=${() => setTagText([...tags, tag].join(", "))}>#${tag}</button>`,
          )}
        </div>`}
    </div>
    ${error && html`<div class="error-text">${error}</div>`}
    <div class="actions">
      <button type="button" class="btn" onClick=${cancel}>${t("취소")}</button>
      <button type="button" class="btn primary" disabled=${trimmedName === ""} onClick=${save}>${saveTitle}</button>
    </div>
  <//>`;
}

/** 새 페이지 · 페이지 편집. onSave(url, alias) 는 저장하고, 실패하면 오류를 던진다. */
export function LinkEditSheet({ original, onSave, onClose }) {
  const [urlText, setUrlText] = useState(original?.url ?? "");
  const urlInput = useRef(null);
  const aliasInput = useRef(null);
  const [alias, setAlias] = useState(original?.alias ?? "");
  const [valid, setValid] = useState(Boolean(original));
  const [error, setError] = useState(null);

  // 주소 규칙은 앱에 있다.
  useEffect(() => {
    let current = true;
    invoke("parse_url", { text: urlText }).then((parsed) => {
      if (current) {
        setValid(parsed != null);
      }
    });
    return () => {
      current = false;
    };
  }, [urlText]);

  const save = async () => {
    try {
      // 입력 칸에서 바로 읽는다 (한글 조합 중 저장 버튼을 눌러도 마지막 글자가 빠지지 않게).
      const finalAlias = aliasInput.current?.value ?? alias;
      await onSave(urlInput.current?.value ?? urlText, finalAlias.trim() === "" ? null : finalAlias);
      onClose();
    } catch (message) {
      setError(String(message));
    }
  };

  return html`<${Sheet} width=${480} onClose=${onClose} onSubmit=${save} canSubmit=${valid}>
    <h2>${original ? t("페이지 편집") : t("새 페이지")}</h2>
    <div class="form">
      <label for="link-url">URL</label>
      <input
        id="link-url"
        ref=${urlInput}
        class="field"
        type="text"
        spellcheck=${false}
        autocapitalize="off"
        placeholder="https://example.com"
        value=${urlText}
        onInput=${(event) => setUrlText(event.target.value)}
      />
      ${urlText !== "" && !valid && html`<span></span><div class="error-text">${t("http(s) URL 형식이 아닙니다.")}</div>`}
      <label for="link-alias">Alias</label>
      <input
        id="link-alias"
        ref=${aliasInput}
        class="field"
        type="text"
        spellcheck=${false}
        placeholder=${t("비워 두면 URL 로 표시")}
        value=${alias}
        onInput=${(event) => setAlias(event.target.value)}
      />
    </div>
    ${error && html`<div class="error-text">${error}</div>`}
    <div class="actions">
      <button type="button" class="btn" onClick=${onClose}>${t("취소")}</button>
      <button type="button" class="btn primary" disabled=${!valid} onClick=${save}>${t("저장")}</button>
    </div>
  <//>`;
}

/** 섹션 이름 입력 (새 섹션 · 이름 바꾸기). onSave(name) 은 저장하고, 실패하면 오류를 던진다. */
export function SectionNameSheet({ title, initial = "", saveTitle = t("저장"), description, onSave, onClose }) {
  const [name, setName] = useState(initial);
  const [error, setError] = useState(null);
  const input = useRef(null);
  const save = async () => {
    // 입력 칸에서 바로 읽는다 (한글 조합 중 저장 버튼을 눌러도 마지막 글자가 빠지지 않게).
    const value = (input.current?.value ?? name).trim();
    if (!value) {
      return;
    }
    try {
      await onSave(value);
      onClose();
    } catch (message) {
      setError(String(message));
    }
  };
  return html`<${Sheet} width=${400} onClose=${onClose} onSubmit=${save} canSubmit=${name.trim() !== ""}>
    <h2>${title}</h2>
    ${description && html`<div class="secondary">${description}</div>`}
    <input ref=${input} class="field" type="text" spellcheck=${false} placeholder=${t("섹션 이름")} value=${name} onInput=${(event) => setName(event.target.value)} />
    ${error && html`<div class="error-text">${error}</div>`}
    <div class="actions">
      <button type="button" class="btn" onClick=${onClose}>${t("취소")}</button>
      <button type="button" class="btn primary" disabled=${name.trim() === ""} onClick=${save}>${saveTitle}</button>
    </div>
  <//>`;
}
