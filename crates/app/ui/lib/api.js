// 앱(Rust)과 주고받는 통로.
//
// 앱 안에서는 Tauri 가 넣어 주는 window.__TAURI__ 를 쓴다.
// 일반 브라우저에서 화면만 열어 볼 때(개발 · 화면 검사)는 dev/mock.js 의 가짜 앱을 쓴다.

const tauri = window.__TAURI__;

export const inApp = Boolean(tauri);

const backend = tauri
  ? {
      invoke: (command, args) => tauri.core.invoke(command, args),
      listen: (event, handler) => tauri.event.listen(event, (message) => handler(message.payload)),
    }
  : (await import("../dev/mock.js")).backend;

/** 앱의 명령을 부른다. 인자 이름은 camelCase 로 적는다 (Rust 쪽 snake_case 와 자동으로 맞춰진다). */
export function invoke(command, args) {
  return backend.invoke(command, args);
}

/** 앱이 보내는 알림을 듣는다. 그만 들으려면 돌려받은 함수를 부른다. */
export function listen(event, handler) {
  const unlisten = backend.listen(event, handler);
  return () => {
    Promise.resolve(unlisten).then((stop) => stop?.());
  };
}

/** 시작할 때 한 번 받는 내용 (앱 이름 · 버전 · 색 · 브라우저 · 운영체제 등) */
export const boot = await invoke("bootstrap");

document.documentElement.dataset.platform = boot.platform;

// 웹 브라우저의 기본 오른쪽 클릭 메뉴(새로고침 · 요소 검사 등)는 앱에 맞지 않아 띄우지 않는다.
// 앱의 메뉴는 각 화면이 따로 띄우고, 글자를 넣는 칸과 고른 글자에서는 복사 · 붙여넣기 메뉴를 그대로 둔다.
document.addEventListener("contextmenu", (event) => {
  const target = event.target instanceof Element ? event.target : null;
  const editable = target?.closest("input, textarea, [contenteditable=''], [contenteditable='true']");
  const hasSelection = !document.getSelection()?.isCollapsed;
  if (!editable && !hasSelection) {
    event.preventDefault();
  }
});
