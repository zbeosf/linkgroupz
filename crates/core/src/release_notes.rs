use crate::update::AppVersion;
use serde::Serialize;
use std::sync::LazyLock;

/// 한 버전의 바뀐 점
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReleaseEntry {
    pub version: AppVersion,
    /// 한국어
    pub changes: Vec<String>,
    /// 영어 (화면에는 `localized` 로 지금 언어 쪽을 보낸다)
    #[serde(skip)]
    pub changes_en: Vec<String>,
}

impl ReleaseEntry {
    fn new(version: &str, changes: &[&str], changes_en: &[&str]) -> Self {
        Self {
            version: AppVersion::parse(version).expect("바뀐 점의 버전은 숫자와 점으로 적는다"),
            changes: changes.iter().map(|change| (*change).to_owned()).collect(),
            changes_en: changes_en.iter().map(|change| (*change).to_owned()).collect(),
        }
    }

    /// 지금 화면 언어의 바뀐 점만 담은 것 ('바뀐 점' 창에 보낸다)
    pub fn localized(&self) -> Self {
        let changes = if crate::i18n::is_english() { &self.changes_en } else { &self.changes };
        Self { version: self.version.clone(), changes: changes.clone(), changes_en: Vec::new() }
    }
}

/// 버전마다 바뀐 점. 업데이트한 뒤 처음 켤 때 '바뀐 점' 창에 보여준다.
///
/// 새 버전을 배포할 때 맨 위에 한 항목을 더한다 (Cargo.toml 의 workspace 버전과 같은 번호).
///
/// 쓰는 사람이 읽는 글이다. 새로 생긴 기능 · 달라진 사용법 · 고친 오류를, 앱을 쓰기만 하는 사람이 아는 말로 짧게 적는다.
/// 만드는 쪽 용어(토큰 · 서명 · 저장소 · 빌드 등)와 내부 구조 이야기는 적지 않는다.
pub struct ReleaseNotes;

static ENTRIES: LazyLock<Vec<ReleaseEntry>> = LazyLock::new(|| {
    vec![ReleaseEntry::new("1.0.0", &[
        "첫 공개 버전입니다. 자주 여는 웹 페이지를 그룹으로 묶어 Chrome · Edge · Whale 에 한 번에 엽니다",
        "그룹마다 색 · 아이콘 · 태그를 붙이고 섹션으로 묶습니다. 페이지 스위치를 끄면 그룹을 열 때 그 페이지는 열지 않습니다",
        "브라우저에 열린 탭 · 탭 그룹을 가져오고, 같은 이름의 탭 그룹과 페이지를 맞춥니다",
        "메뉴바(Windows 는 트레이) 창에서 찾아 바로 엽니다. 키보드 · 오른쪽 클릭 메뉴 · 끌어서 옮기기를 쓸 수 있습니다",
        "보관함, 되돌리기(⌘Z, Windows 는 Ctrl+Z), 프로필, 내보내기 · 가져오기와 하루 한 번 자동 백업을 씁니다",
        "새 버전이 나오면 앱 안에서 진행 막대를 보며 업데이트합니다",
        "한국어 · 영어로 쓸 수 있습니다. 시스템 언어를 따르고, 설정 → 언어에서 바꿉니다",
        "화면 모드는 시스템을 따르고, 설정 → 화면 모드에서 라이트 · 다크로 고를 수 있습니다",
    ], &[
        "The first public release. Group the web pages you open every day and open them in Chrome · Edge · Whale at once",
        "Give each group a color, icon and tags, and fold groups into sections. Turn a page's switch off to skip it when the group opens",
        "Import open tabs and tab groups from the browser, and sync a group with the tab group of the same name",
        "Search and open from the menu bar (the tray on Windows) window, with the keyboard, right-click menus and drag and drop",
        "Archive, undo (⌘Z, Ctrl+Z on Windows), profiles, export and import, and automatic daily backups",
        "Update from inside the app with a progress bar when a new version is out",
        "Available in English and Korean. It follows the system language; change it in Settings → Language",
        "The appearance follows the system; choose Light or Dark in Settings → Appearance",
    ])]
});

impl ReleaseNotes {
    /// 최신이 위
    pub fn entries() -> &'static [ReleaseEntry] {
        &ENTRIES
    }

    /// 지난번에 본 버전(previous) 다음부터 지금 버전까지의 항목. 최신이 위.
    /// previous 를 모르면(이 기능 이전 버전에서 올라온 경우) 지금 버전 항목만 준다.
    pub fn entries_after(previous: Option<&AppVersion>, up_to: &AppVersion, all: &[ReleaseEntry]) -> Vec<ReleaseEntry> {
        all.iter()
            .filter(|entry| {
                if entry.version > *up_to {
                    return false;
                }
                match previous {
                    Some(previous) => entry.version > *previous,
                    None => entry.version == *up_to,
                }
            })
            .cloned()
            .collect()
    }

    /// 켤 때 '바뀐 점' 창에 보여줄 항목. 보여줄 것이 없으면 빈 목록.
    /// - last_seen: 지난번 켤 때 기록한 버전 (처음이면 None)
    /// - is_existing_user: 이전에 앱을 써 본 적이 있는지 (처음 설치한 사람에게는 시작 안내만 보여준다)
    pub fn entries_to_show(
        last_seen: Option<&AppVersion>,
        is_existing_user: bool,
        current: &AppVersion,
        all: &[ReleaseEntry],
    ) -> Vec<ReleaseEntry> {
        match last_seen {
            Some(last_seen) if last_seen < current => Self::entries_after(Some(last_seen), current, all),
            Some(_) => Vec::new(),
            None if is_existing_user => Self::entries_after(None, current, all),
            None => Vec::new(),
        }
    }
}
