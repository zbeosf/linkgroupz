# LinkGroupz 설치 · 세팅 안내

[English](INSTALL.md) | **한국어**

자주 여는 웹 페이지를 그룹으로 모아 두고, 메뉴바(Windows 는 트레이)에서 Chrome · Edge · Whale 에 한 번에 여는 앱입니다. 그룹마다 여는 브라우저를 정합니다 (기본 Chrome).
그림이 들어간 자세한 사용법은 앱의 메뉴바 아이콘 → **안내** → **사용법 보기** 에 있습니다 (macOS DMG 안의 `사용법 · Guide/index.html` 과 같습니다).

필요한 것

- macOS 13(Ventura) 이상, **Apple Silicon(M1 이후) Mac 전용** — Intel Mac 에서는 실행되지 않습니다. 또는 Windows 10 · 11 (64비트)
- Google Chrome · Microsoft Edge · Naver Whale 중 하나

## 1. 설치

### macOS

1. `LinkGroupz-x.y.z.dmg` 를 더블클릭해 엽니다.
2. **LinkGroupz** 를 **Applications** 폴더로 끌어다 놓습니다.
3. 응용 프로그램 폴더에서 LinkGroupz 를 엽니다. 처음에는 시작 안내와 관리 창이 열리고, 메뉴바에 카드 세 장 모양 아이콘이 생깁니다 (Dock 에도 아이콘이 생기며, 설정에서 끌 수 있습니다). 그 뒤로는 켤 때 메뉴바 창이 열립니다.

#### 처음 열 때 "'LinkGroupz'을(를) 열지 않음" 경고가 뜨면

Apple 공증을 받지 않은 앱이라서 뜨는 경고입니다. **'휴지통으로 이동' 은 누르지 말고 '완료'** 를 누른 뒤 한 번만 허용합니다.

1. **시스템 설정 → 개인정보 보호 및 보안** 을 엽니다.
2. 아래쪽 **보안** 의 *Mac을 보호하기 위해 'LinkGroupz'을(를) 차단했습니다.* 옆 **그래도 열기** 를 누릅니다. (경고 창이 뜬 뒤 한 시간쯤만 보이니, 안 보이면 앱을 한 번 더 열어 보세요)
3. Mac 암호를 넣고, 다시 뜨는 창에서 **그래도 열기** 를 누릅니다.

터미널을 쓴다면 아래 명령으로 대신할 수 있습니다.

```bash
xattr -dr com.apple.quarantine /Applications/LinkGroupz.app
```

### Windows

1. `LinkGroupz-x.y.z-setup.exe` 를 실행합니다. 관리자 권한 없이 내 계정에만 설치됩니다.
2. **Windows 의 PC 보호** 창이 뜨면 **추가 정보** → **실행** 을 누릅니다 (서명하지 않은 설치 파일이라서 뜨는 창입니다).
3. 설치가 끝나면 LinkGroupz 를 엽니다. 처음에는 시작 안내와 관리 창이 열리고, 작업 표시줄 오른쪽 트레이에 카드 세 장 모양 아이콘이 생깁니다. 그 뒤로는 켤 때 트레이 창이 열립니다. 아이콘이 안 보이면 ∧ 를 눌러 숨겨진 아이콘에서 찾습니다.

## 2. 세팅

처음 열면 관리 창과 **LinkGroupz 시작하기** 창이 뜹니다. 시작하기 창은 메뉴바(트레이) 아이콘 → **안내** 로 다시 열 수 있습니다.

재부팅해도 자동으로 켜지게 하려면 메뉴바(트레이) 아이콘 → **설정** → **로그인할 때 자동으로 실행** 을 켭니다.

아이콘을 누르지 않고 키보드로 메뉴바(트레이) 창을 열고 싶으면 **설정** → **단축키** 의 **없음** 을 누른 뒤 쓸 단축키를 누릅니다 (예: `⌥Space`, `Ctrl+Alt+K`). 다른 프로그램을 쓰는 중에도 그 단축키로 창이 열리고, 바로 검색할 수 있습니다.

### 확장 설치

설치하면 그룹을 이름 · 색이 붙은 탭 그룹으로 열고, 열린 탭 · 탭 그룹을 가져올 수 있습니다. 쓰는 브라우저(Chrome · Edge · Whale)마다 넣습니다.

- **macOS**: 권장. 확장이 없어도 열린 탭은 가져올 수 있지만(탭 그룹 정보 없이), 확장이 있으면 브라우저 창을 건드리지 않고 가져옵니다.
- **Windows**: 열린 탭 · 탭 그룹을 가져오려면 필요합니다.

1. Chrome 주소창에 `chrome://extensions` 입력
2. 오른쪽 위 **개발자 모드** 켜기
3. **압축해제된 확장 프로그램 로드** → 아래 폴더 선택 (시작 안내 창의 **확장 폴더 보기** 로 열 수 있습니다)
   - macOS: `~/Library/Application Support/LinkGroupz/chrome-extension` (폴더 고르는 창에서 `⌘⇧G` 후 경로를 붙여 넣어도 됩니다)
   - Windows: `%APPDATA%\LinkGroupz\chrome-extension`
4. 시작 안내 창에 **확장 설치됨** 이 뜨면 끝

Edge: 주소창에 `edge://extensions` → **개발자 모드** 켜기 → **압축을 푼 확장 로드** → 같은 폴더 선택.

Whale: 주소창에 `whale://extensions` → **개발자 모드** 켜기 → **압축해제 된 확장앱 설치** → 같은 폴더 선택.

### macOS 가 물어보면 허용

| 물어보는 창 | 언제 | 누를 것 |
|---|---|---|
| 'Google Chrome'(또는 'Microsoft Edge' · 'Whale') 을 제어하려고 합니다 | 열린 탭 가져오기를 그 브라우저에서 처음 할 때 (LinkGroupz 확장이 탭 목록을 미리 보내 둔 경우에는 묻지 않습니다) | 허용 |
| 키체인 'LinkGroupz 미리보기 로그인 쿠키' 접근 (영어 화면이면 'LinkGroupz Preview Sign-in Cookies') | 미리보기에서 로그인한 뒤 다시 미리보기를 열 때, 설정의 **저장된 로그인 확인** 을 누를 때 (앱을 업데이트하면 다시 물을 수 있음) | Mac 로그인 암호 입력 → **항상 허용** |

## 3. 업데이트 · 삭제

- **자동 업데이트**: 업데이트 주소를 넣어 빌드한 앱에서 쓸 수 있습니다. 1시간마다 새 버전이 올라왔는지 확인해, 있으면 업데이트 창에서 묻습니다 (**지금 업데이트 / 나중에**). 업데이트하면 진행 막대가 차오른 뒤 앱이 다시 켜집니다. 설정 → **업데이트** 의 ↻ 로 직접 확인할 수도 있습니다. 내려받은 파일은 서명을 확인한 뒤에만 설치합니다. 자동 확인은 설정 → **업데이트** → **새 버전 자동 확인** 에서 끌 수 있습니다. 업데이트한 뒤에는 확장을 넣은 브라우저의 확장 페이지(`chrome://extensions` · `edge://extensions` · `whale://extensions`) 의 LinkGroupz Tab Groups 에서 새로고침(⟳)을 한 번 눌러 주세요.
- **직접 업데이트**: 새 DMG 의 LinkGroupz 를 Applications 에 덮어쓰거나(macOS), 새 설치 파일을 실행합니다(Windows). 그 뒤 확장을 넣은 브라우저의 확장 페이지에서 LinkGroupz Tab Groups 를 새로고침(⟳). 데이터 · 로그인할 때 자동 실행 설정은 그대로 남습니다.
- **삭제**
  - macOS: 메뉴바 아이콘 → **종료** → Applications 의 LinkGroupz 를 휴지통으로 → Chrome · Edge · Whale 에서 확장 제거. 데이터까지 지우려면 `~/Library/Application Support/LinkGroupz` 폴더와 키체인 항목 'LinkGroupz 미리보기 로그인 쿠키' 를 지웁니다.
  - Windows: 트레이 아이콘 → **종료** → 설정 → 앱 → 설치된 앱에서 LinkGroupz 제거 → Chrome · Edge · Whale 에서 확장 제거. 데이터까지 지우려면 `%APPDATA%\LinkGroupz` 폴더를 지웁니다.

## 데이터 위치

데이터 폴더는 macOS `~/Library/Application Support/LinkGroupz`, Windows `%APPDATA%\LinkGroupz` 입니다.

| 무엇 | 위치 |
|---|---|
| 그룹 · 페이지 · 보관함 | 데이터 폴더의 `data.json` |
| 설정 | 데이터 폴더의 `settings.json` |
| Chrome · Edge · Whale 확장 | 데이터 폴더의 `chrome-extension` |
| 그룹 아이콘 이미지 | 데이터 폴더의 `icons` |
| 사이트 아이콘 | 데이터 폴더의 `favicons` |
| 미리보기 로그인 쿠키 | macOS: 키체인 'LinkGroupz 미리보기 로그인 쿠키'. Windows: 데이터 폴더의 `preview-profile` (WebView2 가 암호화해 보관) |

`data.json` 을 복사해 두면 백업이 되고, 다른 컴퓨터의 같은 위치에 두면 그대로 옮겨집니다 (앱을 끈 상태에서).
원본 LinkGroups 의 `data.json`(`~/Library/Application Support/LinkGroups/data.json`)도 같은 형식이라, LinkGroupz 데이터 폴더에 복사하면 그대로 읽힙니다.

## 오픈소스 라이선스

LinkGroupz 에 들어 있는 외부 코드의 저작권 · 라이선스 고지는 설정 → **오픈소스 라이선스** → **보기** 에서 볼 수 있습니다.
