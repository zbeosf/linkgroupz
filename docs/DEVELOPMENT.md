# 개발 안내

LinkGroupz 를 고치거나 빌드 · 배포하는 사람을 위한 문서입니다. 앱 소개와 설치는 [README](../README.ko.md), 사용법은 [사용법](guide/index.html)에 있습니다.

## 구조

| 경로 | 내용 |
|---|---|
| `crates/core/` | 화면과 무관한 부분 (`linkgroupz-core`): 모델 · JSON 저장소 · 검색 · 브라우저(Chrome · Edge · Whale) 실행 · 확장 감지 · 탭 스냅샷 · 닫힌 탭 그룹(브라우저 LevelDB 읽기) · 섹션 · 키체인 쿠키 · 사이트 아이콘 주소 · 그룹 아이콘 이미지 · 보관함 · 업데이트 규칙. 운영체제마다 다른 부분은 `src/platform/` |
| `crates/app/` | 앱 (`linkgroupz`, [Tauri 2](https://tauri.app)): 메뉴바 · 트레이 아이콘, 창, 미리보기, 자동 업데이트, 화면이 부르는 명령(`src/commands.rs`) |
| `crates/app/ui/` | 화면. 묶음 도구 없이 브라우저가 바로 읽는 HTML · CSS · JS ([Preact](https://preactjs.com) + htm). `dev/mock.js` 는 화면을 일반 브라우저에서 열어 볼 때 쓰는 가짜 앱 |
| `crates/host/` | Chrome · Edge · Whale 확장이 Native Messaging 으로 부르는 호스트 (`linkgroupz-host`). 탭 목록을 브라우저별로 앱 데이터 폴더에 저장 |
| `chrome-extension/` | 확장 (MV3, Chrome · Edge · Whale 공용): 탭 그룹 만들기, 탭 · 탭 그룹 목록 전달 |
| `xtask/` | 빌드 보조 도구: 아이콘 · 배포 파일 · 라이선스 고지 · 릴리스 묶음 만들기 (`cargo xtask`) |
| `assets/` | 앱 · 메뉴바 아이콘 원본(SVG) |
| `docs/guide/` | 사용법 HTML · 스크린샷 |

### 확장과 앱이 주고받는 방식

Chrome · Edge · Whale 은 모두 Chromium 이라 같은 확장을 쓴다. 다른 것은 실행 파일 · 프로필 폴더뿐이라 `Browser`(`crates/core/src/browser.rs`)에 모았다.

- **그룹 열기**: 앱이 `https://linkgroupz.invalid/open#<base64url JSON>` 을 그룹의 브라우저로 열면 확장이 가로채 탭 그룹을 만들고 그 탭을 닫는다. `.invalid` 는 접속되지 않는 예약 도메인이라 확장이 없을 때 요청이 밖으로 나가지 않는다.
- **탭 · 탭 그룹 목록**: 확장이 탭이 바뀔 때마다 Native Messaging 호스트(`dev.linkgroupz.snapshot`)로 목록을 보내고, 호스트가 스냅샷의 `browser` 값에 따라 앱 데이터 폴더의 `chrome-snapshot.json` 또는 `edge-snapshot.json` 에 쓴다. 앱은 이 파일만 읽는다. 호스트 등록은 앱이 실행될 때 확장이 감지된 브라우저마다 한다 (macOS 는 브라우저 프로필 폴더의 `NativeMessagingHosts/`, Windows 는 레지스트리).
- 파일이 없으면 마커 탭을 잠깐 열어 받아 온다. macOS 는 `/snapshot#<nonce>` 를 열고 AppleScript 로 결과를 읽고, AppleScript 가 없는 Windows 는 `/push#<nonce>` 를 열어 확장이 호스트로 보내게 한다.
- **확장 감지** (`ChromeExtensionDetector`): 브라우저 프로필의 `Secure Preferences` · `Preferences` 에서 압축해제된 확장을 찾는다. 앱이 확장을 복사해 두는 곳(앱 데이터 폴더의 `chrome-extension`)에서 로드했으면 경로만 보고 알고, 다른 곳이면 브라우저가 기록한 권한(nativeMessaging · tabGroups)이 맞고 macOS 가 권한을 묻는 폴더(문서 · 데스크탑 · 다운로드 · iCloud 등) 밖인 것만 `manifest.json` 이름을 확인한다. 다른 확장 폴더를 열면 macOS 가 그 폴더 권한을 묻기 때문이다.
- 원본 LinkGroups 확장과는 주고받지 않는다. LinkGroupz 확장만 쓴다.

### 원작 LinkGroups(macOS · Swift)와 다른 점 · 호환

- macOS 전용 → macOS · Windows. 화면은 SwiftUI 대신 웹뷰(macOS WKWebView, Windows WebView2)에 그린다.
- `data.json` 은 원본과 같은 형식이다 (대문자 UUID, Swift `Date` 형식의 시각). 원본의 `data.json` 을 LinkGroupz 데이터 폴더에 복사하면 그대로 읽힌다.
- 그룹 아이콘 이름은 원본과 같은 SF Symbol 이름을 그대로 쓰고, 그림은 [Phosphor Icons](https://phosphoricons.com) 로 그린다.
- 미리보기는 페이지 줄 옆에 뜨는 별도 창이다. 로그인 쿠키는 macOS 에서는 원본처럼 키체인에만 두고, Windows 에서는 WebView2 프로필 폴더에 둔다.
- 자동 업데이트는 위키 첨부 파일 + 개인 토큰 대신 GitHub 릴리스 + 서명 확인으로 한다 (토큰이 필요 없다).
- Windows 에서는 AppleScript 가 없어 확장이 있어야 열린 탭을 가져올 수 있다.

## 언어 (한국어 · 영어)

- 화면 글은 한국어로 적고 `t("한국어")` 로 감싼다 (`crates/app/ui/lib/i18n.js`). 영어는 `crates/app/ui/lib/i18n-en.js` 의 표에서 한국어 글을 열쇠로 찾는다. 자리 표시는 `{0}` `{1}` …, 단수 · 복수가 갈리면 표의 값을 함수로 둔다.
- 화면 글을 더하거나 고치면 `node scripts/check-i18n.mjs` 로 영어 표에 빠진 글이 없는지 본다.
- 앱 안쪽(Rust) 글은 `tr!("한국어", "English")` 로 두 말을 함께 적는다 (`crates/core/src/i18n.rs`). 오류 종류는 `#[error("{}", crate::tr!(…))]`.
- 언어는 설정 → 언어(시스템 · 한국어 · English). 시스템이면 운영체제 언어가 한국어일 때만 한국어다. 바꾸면 모든 창이 다시 그려진다.
- 바뀐 점(`release_notes.rs`)은 버전마다 한국어 · 영어를 함께 적는다. 릴리스의 `latest.json` notes 에는 `### English` 줄 뒤에 영어가 붙고, 업데이트 창이 화면 언어 쪽을 보여 준다.
- 사용법은 한국어 `docs/guide/index.html`, 영어 `docs/guide/en/index.html` (그림은 `en/images`, macOS 시스템 창 그림은 한국어판 것을 함께 쓴다). 앱은 화면 언어에 맞는 쪽을 연다. 설치 안내는 영어 `INSTALL.md`, 한국어 `INSTALL.ko.md`.

## 개발

Rust 1.90 이상. macOS 는 Xcode Command Line Tools, Windows 는 [Tauri 의 준비물](https://tauri.app/start/prerequisites/)(MSVC 빌드 도구, WebView2)이 필요하다.

```bash
cargo test --workspace          # 검사 (원본의 CoreChecks 를 옮긴 것은 crates/core/tests/core_checks.rs)
cargo xtask host                # 호스트를 빌드해 앱이 함께 넣을 자리(crates/app/binaries/)에 둔다 — 앱을 처음 빌드하기 전에 한 번
cargo run -p linkgroupz         # 개발 빌드로 실행
```

실제 브라우저 · 운영체제를 건드리는 수동 확인: `cargo run -p linkgroupz-core --example manual -- --help`

예시 데이터로 따로 실행 (실제 데이터를 건드리지 않고, 실행 중인 LinkGroupz 와 별개로 뜬다):

```bash
LINKGROUPZ_DATA_DIR=/tmp/linkgroupz-sample cargo run -p linkgroupz -- --welcome
```

화면만 브라우저에서 보기 (가짜 앱 `dev/mock.js` 가 대신 답한다. 옵션은 그 파일 맨 위 참고):

```bash
python3 -m http.server 8000 --bind 127.0.0.1 --directory crates/app/ui
# http://127.0.0.1:8000/index.html?scene=groupEdit
```

아이콘을 바꿨으면 `cargo xtask icons` (`assets/*.svg` → `crates/app/icons/`), 화면 아이콘을 더하거나 뺐으면 `node scripts/update-icons.mjs`.

## 배포용 설치 파일 만들기

```bash
cargo xtask dist              # macOS: dist/LinkGroupz-<버전>.dmg, Windows: dist/LinkGroupz-<버전>-setup.exe
cargo xtask dist --universal  # macOS: Apple Silicon · Intel 을 합친 앱 (rustup 으로 두 타깃을 모두 설치해 둬야 한다)
cargo xtask dist --install    # macOS: 만든 앱을 /Applications 에 설치
```

Tauri CLI 가 필요하다. `cargo tauri` 가 없으면 미리 빌드된 npm 패키지(`npx @tauri-apps/cli`)를 쓰므로 Node.js 만 있으면 된다.

버전은 [Cargo.toml](../Cargo.toml) 의 `workspace.package.version` 한 곳에서 바꾼다. 새 버전마다 `crates/core/src/release_notes.rs` 맨 위에 바뀐 점을 더한다 (없으면 검사가 실패한다).

macOS DMG 에는 앱 · Applications 바로가기 · 사용법 폴더가 들어 있다. Apple 개발자 인증서가 없어 ad-hoc 서명만 하므로, 받는 사람은 처음 열 때 **시스템 설정 → 개인정보 보호 및 보안 → 그래도 열기** 가 한 번 필요하다 (INSTALL.md 에 안내).

## 자동 업데이트

앱은 [Tauri 업데이터](https://tauri.app/plugin/updater/)로 GitHub 릴리스에서 새 버전을 받는다. 켠 뒤 30초, 그 뒤 1시간마다 이 저장소의 최신 릴리스에 있는 `latest.json` 을 읽고, 지금 버전보다 높으면 업데이트 창(`crates/app/ui/update.html`)에 바뀐 점과 **지금 업데이트 / 나중에** 를 보여 준다 (설정에서 자동 확인을 끌 수 있다). 업데이트하면 같은 창에 내려받기 · 설치 진행 막대가 나오고, 실패하면 이유를 보여 준다. macOS 기본 알림 창은 쓰지 않는다.

- 릴리스를 받을 저장소 이름과 서명 공개 키는 소스에 두지 않는다 (포크해서 따로 배포할 때 소스를 고치지 않아도 되게). [update-source.env.example](../update-source.env.example) 을 `update-source.env` 로 복사해 값을 채우고 빌드하면 앱에 들어간다 (`update-source.env` 는 git 에 올리지 않는다. 같은 이름의 환경 변수로 줘도 된다). 값 없이 빌드한 앱은 자동 업데이트 없이 동작한다.
- 내려받은 파일은 공개 키로 서명을 확인한 뒤에만 설치한다. 서명에는 버전도 들어 있어(`requireSignedVersion`), `latest.json` 을 바꿔 예전 파일을 새 버전인 것처럼 깔게 할 수 없다.
- macOS 는 앱(.app)을 통째로 바꿔 넣고 다시 켠다. Windows 는 설치 프로그램을 창 없이(`/S /R /UPDATE`, tauri.conf.json 의 `installMode: quiet`) 돌리고 끝나면 다시 켠다. 어느 쪽이든 새 버전으로 바뀌려면 앱을 다시 켜야 한다.
- 시험: 디버그 빌드는 `LINKGROUPZ_UPDATE_ENDPOINT` 에 http 주소도 받는다. 로컬에 `latest.json` 과 파일을 두면 진행 창까지 볼 수 있다 (서명이 가짜면 끝에서 설치를 거부한다). `LINKGROUPZ_UPDATE_AUTO_INSTALL=1` 은 묻지 않고 바로 설치한다.
- 시험: 환경 변수 `LINKGROUPZ_UPDATE_ENDPOINT` 로 `latest.json` 주소를 바꿀 수 있다 (서명은 그대로 확인한다. 릴리스 빌드는 https 주소만 받는다).

### 저장소

소스와 배포 파일(릴리스의 DMG · 설치 파일 · 업데이터용 파일 · `latest.json`)을 이 공개 저장소 하나에 둔다. 앱은 로그인 없이 이 저장소의 릴리스에서 새 버전을 받는다 (`update-source.env` 의 `LINKGROUPZ_RELEASES_REPO`). Windows 설치 파일은 GitHub Actions 가 만든다.

### 새 버전 내기

처음 한 번 준비할 것

1. 릴리스를 올릴 공개 저장소를 정한다 (이 저장소를 그대로 쓴다. 포크했으면 자기 저장소).
2. 서명 키를 만든다: `npx @tauri-apps/cli signer generate -w ~/.tauri/linkgroupz-updater.key`. 비밀 키는 릴리스를 만드는 Mac 에만 두고 GitHub 에는 올리지 않는다 — **이 키를 잃으면 이미 설치된 앱에 업데이트를 보낼 수 없으니** 안전한 곳에 따로 보관한다.
3. `update-source.env` 에 저장소 이름과 공개 키(`.pub` 파일의 내용)를 적는다.
4. 같은 두 값을 이 저장소의 Actions 변수 `LINKGROUPZ_RELEASES_REPO` · `LINKGROUPZ_UPDATE_PUBKEY` 로 넣는다 (Windows 설치 파일에 들어간다): `gh variable set LINKGROUPZ_RELEASES_REPO --body <소유자/이름>`

새 버전마다

```bash
# 1. Cargo.toml 의 버전을 올리고, crates/core/src/release_notes.rs 맨 위에 바뀐 점을 더한다
# 2. 의존성이 바뀌었으면 라이선스 고지를 다시 만든다
cargo xtask licenses
# 3. 커밋하고 버전 태그를 올린다 → GitHub Actions(.github/workflows/windows.yml)가 Windows 설치 파일을 만든다
git tag v<버전> && git push origin main v<버전>
# 4. Actions 가 끝나면: macOS 앱 빌드 → Windows 설치 파일 내려받기 → 서명 → latest.json → dist/release-<버전>/
cargo xtask release
# 5. 묶음을 확인한 뒤 릴리스로 올린다
cargo xtask release --publish
```

`cargo xtask release --mac-only` 는 Windows 설치 파일 없이, `--windows <setup.exe>` 는 직접 준 파일로 만든다. macOS 앱은 릴리스를 만드는 Mac 의 CPU 용으로만 만들어진다 (지금은 Apple Silicon 전용 — 릴리스 설명 · INSTALL.md · 사용법에 적어 둔다).

## 라이선스 고지

앱에 들어가는 외부 코드(Rust 크레이트, 화면의 Preact · htm · Phosphor Icons)의 저작권 문구와 라이선스 전문은 [THIRD-PARTY-LICENSES.html](../THIRD-PARTY-LICENSES.html) 에 있습니다. 앱에도 들어 있어 설정 → **오픈소스 라이선스** 에서 열립니다.

- 이 파일은 `cargo xtask licenses` 가 만든다 ([cargo-about](https://github.com/EmbarkStudios/cargo-about) 필요: `cargo install --locked cargo-about --features cli`). 의존성을 바꿨으면 다시 만들어 함께 커밋한다. `cargo xtask licenses --check` 는 파일이 지금 의존성과 같은지만 본다 (`cargo xtask release` 가 먼저 확인한다).
- 받아들이는 라이선스는 [about.toml](../about.toml) 의 `accepted` 에 적는다. 목록에 없는 라이선스(GPL 등)의 크레이트가 들어오면 생성이 실패한다.
- 알려진 취약점 점검: `cargo audit` (`cargo install --locked cargo-audit`)
- 화면에 파일째 넣은 외부 코드는 [crates/app/ui/vendor/README.md](../crates/app/ui/vendor/README.md) 참고.
