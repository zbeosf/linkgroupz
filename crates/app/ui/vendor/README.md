# 함께 넣은 외부 코드 · 그림

화면(`ui/`)은 묶음 도구(번들러) 없이 브라우저가 바로 읽는 파일로 되어 있어, 아래 외부 코드를 파일째 넣어 두었다.

아래 라이선스 파일은 앱에 넣는 고지(`THIRD-PARTY-LICENSES.html`, `cargo xtask licenses`)에도 실린다. 여기에 더하거나 버전을 바꾸면 `xtask/src/licenses.rs` 의 `UI_COMPONENTS` 도 맞춘다.

| 파일 | 무엇 | 라이선스 |
|---|---|---|
| `preact-htm.js` | [htm](https://github.com/developit/htm) 3.1.1 의 `preact/standalone.module.js` — [Preact](https://preactjs.com) 10 과 htm 을 한 파일로 묶은 것 | htm: Apache-2.0 (`LICENSE-htm.txt`), Preact: MIT (`LICENSE-preact.txt`) |
| `../lib/icon-data.js` 의 아이콘 경로 | [Phosphor Icons](https://phosphoricons.com) 2.1.1 (`scripts/update-icons.mjs` 로 받는다) | MIT (`LICENSE-phosphor-icons.txt`) |
