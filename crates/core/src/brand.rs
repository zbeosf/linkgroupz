//! 앱 이름과 거기서 나온 식별자를 한곳에 모은다.
//!
//! 원본 LinkGroups(Swift)와 나란히 깔아 쓸 수 있도록 이름 · 데이터 폴더 · 키체인 · 확장 · 마커 주소를 모두 다르게 둔다.
//! 원본 앱 · 원본 확장과는 주고받지 않는다 (원본 확장 폴더를 읽으면 macOS 가 문서 폴더 권한을 묻기도 했다).
//! 이름을 바꿀 때는 이 파일과 함께 아래도 맞춘다.
//! - `chrome-extension/manifest.json` 의 name, `chrome-extension/background.js` 의 MARKER_HOST · NATIVE_HOST
//! - `crates/app/tauri.conf.json` 의 productName · identifier
//! - `crates/app/build.rs` · `xtask` 의 호스트 실행 파일 이름

/// 화면 · 설치 파일에 보이는 이름
pub const APP_NAME: &str = "LinkGroupz";

/// macOS 번들 식별자 · Windows 앱 식별자
pub const APP_IDENTIFIER: &str = "dev.linkgroupz.LinkGroupz";

/// 앱 데이터 폴더 이름 (macOS `~/Library/Application Support/<이름>`, Windows `%APPDATA%\<이름>`)
pub const DATA_DIR_NAME: &str = "LinkGroupz";

/// 확장이 가로채는 마커 주소의 호스트. `.invalid` 는 접속되지 않는 예약 도메인이다.
pub const MARKER_HOST: &str = "linkgroupz.invalid";

/// Native Messaging 호스트 이름 (브라우저가 호스트 등록 파일을 찾는 이름)
pub const NATIVE_HOST_NAME: &str = "dev.linkgroupz.snapshot";

/// 앱과 함께 설치되는 호스트 실행 파일 이름 (Windows 는 뒤에 `.exe`)
pub const HOST_EXECUTABLE_NAME: &str = "linkgroupz-host";

/// 확장 manifest.json 의 name. 브라우저 프로필에서 확장을 찾을 때 이 이름으로 확인한다.
pub const EXTENSION_MANIFEST_NAME: &str = "LinkGroupz Tab Groups";

/// 미리보기 로그인 쿠키를 넣는 키체인 서비스 이름
pub const COOKIE_VAULT_SERVICE: &str = "dev.linkgroupz.preview-cookies";

/// 페이지를 다른 그룹으로 끌어다 놓을 때 싣는 문자열의 머리
pub const DRAG_PAYLOAD_PREFIX: &str = "linkgroupz-links:";

/// 환경 변수 이름
pub mod env {
    /// 앱 데이터 폴더를 바꾼다 (예시 데이터로 띄울 때)
    pub const DATA_DIR: &str = "LINKGROUPZ_DATA_DIR";
    /// 빌드할 때: 배포 파일이 올라오는 GitHub 저장소 (`소유자/이름`)
    pub const RELEASES_REPO: &str = "LINKGROUPZ_RELEASES_REPO";
    /// 빌드할 때: 받은 파일의 서명을 확인할 공개 키
    pub const UPDATE_PUBKEY: &str = "LINKGROUPZ_UPDATE_PUBKEY";
    /// 시험용: 새 버전을 확인하는 주소(latest.json)를 바꾼다
    pub const UPDATE_ENDPOINT: &str = "LINKGROUPZ_UPDATE_ENDPOINT";
    /// 사용법 스크린샷용 화면 상태
    pub const SCREENSHOT_SCENE: &str = "LINKGROUPZ_SCREENSHOT_SCENE";
}
