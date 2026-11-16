use crate::tr;
use crate::archive::LinkArchive;
use crate::browser::Browser;
use crate::group_icon::GroupIcon;
use crate::ids::Id;
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;
use url::{Position, Url};

/// 그룹 색. 앞의 9가지는 Chrome 탭 그룹 색과 이름이 같고, 나머지는 앱에서만 쓰는 색이다.
/// Chrome 탭 그룹으로 열 때는 `chrome_color` (가장 가까운 Chrome 색)를 넘긴다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupColor {
    // Chrome 탭 그룹 색
    Grey,
    Blue,
    Red,
    Yellow,
    Green,
    Pink,
    Purple,
    Cyan,
    Orange,
    // 앱 전용 색
    Navy,
    Indigo,
    Teal,
    Mint,
    Lime,
    Gold,
    Coral,
    Brown,
    Magenta,
    Black,
}

impl GroupColor {
    pub const ALL: [GroupColor; 19] = [
        GroupColor::Grey,
        GroupColor::Blue,
        GroupColor::Red,
        GroupColor::Yellow,
        GroupColor::Green,
        GroupColor::Pink,
        GroupColor::Purple,
        GroupColor::Cyan,
        GroupColor::Orange,
        GroupColor::Navy,
        GroupColor::Indigo,
        GroupColor::Teal,
        GroupColor::Mint,
        GroupColor::Lime,
        GroupColor::Gold,
        GroupColor::Coral,
        GroupColor::Brown,
        GroupColor::Magenta,
        GroupColor::Black,
    ];

    /// 파일에 적는 이름 (`"blue"`)
    pub fn raw_value(self) -> &'static str {
        match self {
            GroupColor::Grey => "grey",
            GroupColor::Blue => "blue",
            GroupColor::Red => "red",
            GroupColor::Yellow => "yellow",
            GroupColor::Green => "green",
            GroupColor::Pink => "pink",
            GroupColor::Purple => "purple",
            GroupColor::Cyan => "cyan",
            GroupColor::Orange => "orange",
            GroupColor::Navy => "navy",
            GroupColor::Indigo => "indigo",
            GroupColor::Teal => "teal",
            GroupColor::Mint => "mint",
            GroupColor::Lime => "lime",
            GroupColor::Gold => "gold",
            GroupColor::Coral => "coral",
            GroupColor::Brown => "brown",
            GroupColor::Magenta => "magenta",
            GroupColor::Black => "black",
        }
    }

    pub fn from_raw_value(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|color| color.raw_value() == raw)
    }

    pub fn display_name(self) -> &'static str {
        match self {
            GroupColor::Grey => tr!("회색", "Grey"),
            GroupColor::Blue => tr!("파랑", "Blue"),
            GroupColor::Red => tr!("빨강", "Red"),
            GroupColor::Yellow => tr!("노랑", "Yellow"),
            GroupColor::Green => tr!("초록", "Green"),
            GroupColor::Pink => tr!("분홍", "Pink"),
            GroupColor::Purple => tr!("보라", "Purple"),
            GroupColor::Cyan => tr!("청록", "Cyan"),
            GroupColor::Orange => tr!("주황", "Orange"),
            GroupColor::Navy => tr!("남색", "Navy"),
            GroupColor::Indigo => tr!("인디고", "Indigo"),
            GroupColor::Teal => tr!("틸", "Teal"),
            GroupColor::Mint => tr!("민트", "Mint"),
            GroupColor::Lime => tr!("라임", "Lime"),
            GroupColor::Gold => tr!("금색", "Gold"),
            GroupColor::Coral => tr!("코랄", "Coral"),
            GroupColor::Brown => tr!("갈색", "Brown"),
            GroupColor::Magenta => tr!("자홍", "Magenta"),
            GroupColor::Black => tr!("검정", "Black"),
        }
    }

    /// Chrome 탭 그룹이 지원하는 9가지 중 가장 가까운 색
    pub fn chrome_color(self) -> GroupColor {
        match self {
            GroupColor::Grey
            | GroupColor::Blue
            | GroupColor::Red
            | GroupColor::Yellow
            | GroupColor::Green
            | GroupColor::Pink
            | GroupColor::Purple
            | GroupColor::Cyan
            | GroupColor::Orange => self,
            GroupColor::Navy => GroupColor::Blue,
            GroupColor::Indigo => GroupColor::Purple,
            GroupColor::Teal => GroupColor::Cyan,
            GroupColor::Mint => GroupColor::Cyan,
            GroupColor::Lime => GroupColor::Green,
            GroupColor::Gold => GroupColor::Yellow,
            GroupColor::Coral => GroupColor::Red,
            GroupColor::Brown => GroupColor::Orange,
            GroupColor::Magenta => GroupColor::Pink,
            GroupColor::Black => GroupColor::Grey,
        }
    }

    pub fn is_chrome_color(self) -> bool {
        self.chrome_color() == self
    }

    /// 화면에 칠하는 색 (sRGB 0–255). 앞의 9가지는 Chrome 탭 그룹 색에 가깝게 맞춘 값
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            GroupColor::Grey => (94, 99, 105),
            GroupColor::Blue => (26, 115, 232),
            GroupColor::Red => (217, 48, 38),
            GroupColor::Yellow => (250, 171, 0),
            GroupColor::Green => (23, 128, 56),
            GroupColor::Pink => (209, 23, 135),
            GroupColor::Purple => (161, 66, 240),
            GroupColor::Cyan => (0, 133, 143),
            GroupColor::Orange => (250, 125, 0),
            GroupColor::Navy => (28, 56, 135),
            GroupColor::Indigo => (89, 87, 214),
            GroupColor::Teal => (48, 176, 199),
            GroupColor::Mint => (0, 199, 166),
            GroupColor::Lime => (140, 194, 26),
            GroupColor::Gold => (204, 158, 33),
            GroupColor::Coral => (255, 115, 102),
            GroupColor::Brown => (140, 94, 56),
            GroupColor::Magenta => (219, 56, 204),
            GroupColor::Black => (33, 33, 38),
        }
    }
}

/// 모르는 색 이름(이후 버전에서 추가된 색 등)이 있어도 데이터 파일 전체를 못 읽는 일이 없도록 회색으로 읽는다.
impl<'de> Deserialize<'de> for GroupColor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Ok(GroupColor::from_raw_value(&raw).unwrap_or(GroupColor::Grey))
    }
}

/// 페이지 주소. 저장된 문자열을 그대로 들고 있고, 필요할 때만 풀어서 본다.
///
/// 주소를 다시 조립하면 사용자가 적은 모양(`https://a.com` 과 `https://a.com/`)이 바뀌므로 문자열을 원본으로 삼는다.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LinkUrl(String);

impl LinkUrl {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parsed(&self) -> Option<Url> {
        Url::parse(&self.0).ok()
    }

    /// 소문자 스킴 (`https`)
    pub fn scheme(&self) -> Option<String> {
        self.parsed().map(|url| url.scheme().to_owned())
    }

    pub fn host(&self) -> Option<String> {
        self.parsed()
            .and_then(|url| url.host_str().map(str::to_owned))
            .filter(|host| !host.is_empty())
    }

    /// 주소에 적힌 포트 (스킴의 기본 포트면 없음)
    pub fn port(&self) -> Option<u16> {
        self.parsed().and_then(|url| url.port())
    }

    /// % 인코딩을 푼 경로. 끝의 `/` 는 뺀다 (루트는 `/`).
    pub fn path(&self) -> String {
        let Some(url) = self.parsed() else {
            return String::new();
        };
        let mut path = percent_decode_str(url.path()).decode_utf8_lossy().into_owned();
        if path.len() > 1 && path.ends_with('/') {
            path.pop();
        }
        path
    }

    pub fn is_file(&self) -> bool {
        self.scheme().as_deref() == Some("file")
    }

    /// 브라우저에 넘겨도 되는 주소인지: 풀리는 주소이고 http · https · file 이어야 한다.
    ///
    /// 저장된 글자는 그대로 브라우저의 실행 인자가 된다. 검사하지 않으면 data.json 에 `--renderer-cmd-prefix=…` 같은
    /// 글자가 든 항목이 브라우저 실행 옵션으로 읽힌다 (백업 파일을 다른 사람에게 받아 쓰는 경우).
    pub fn is_openable(&self) -> bool {
        self.parsed().is_some_and(|url| ["http", "https", "file"].contains(&url.scheme()))
    }
}

impl std::fmt::Display for LinkUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SavedLink {
    pub id: Id,
    pub url: LinkUrl,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// 그룹을 열 때 이 페이지도 열지 (페이지 줄의 스위치, 기본 켬).
    /// 원본 LinkGroups 1.5.8 과 같은 이름으로 적고, 켜져 있으면 적지 않아 이전 형식과 같다.
    #[serde(rename = "opensWithGroup", default = "opens_with_group_default", skip_serializing_if = "is_opens_with_group_default")]
    pub opens_with_group: bool,
}

fn opens_with_group_default() -> bool {
    true
}

fn is_opens_with_group_default(value: &bool) -> bool {
    *value
}

impl SavedLink {
    pub fn new(url: LinkUrl, alias: Option<&str>) -> Self {
        Self { id: Id::new(), url, alias: Self::normalized_alias(alias), opens_with_group: true }
    }

    pub fn display_name(&self) -> String {
        if let Some(alias) = self.alias.as_deref().filter(|alias| !alias.is_empty()) {
            return alias.to_owned();
        }
        let host = self.url.host().unwrap_or_else(|| self.url.as_str().to_owned());
        let path = self.url.path();
        if path.is_empty() || path == "/" {
            return host;
        }
        host + &path
    }

    pub fn normalized_alias(alias: Option<&str>) -> Option<String> {
        let trimmed = alias?.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(trimmed.to_owned())
    }

    /// 사용자가 입력한 문자열을 http(s) · file URL 로 바꾼다. 스킴이 없으면 https 를 붙인다.
    pub fn parse_url(text: &str) -> Option<LinkUrl> {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.contains(' ') {
            return None;
        }
        let candidate = if trimmed.contains("://") { trimmed.to_owned() } else { format!("https://{trimmed}") };
        let url = Url::parse(&candidate).ok()?;
        let scheme = url.scheme();
        if !["http", "https", "file"].contains(&scheme) {
            return None;
        }
        if scheme != "file" && url.host_str().is_none_or(str::is_empty) {
            return None;
        }
        let mut text = url.as_str().to_owned();
        // 경로 없이 적은 주소(https://a.com)에 URL 파서가 붙인 루트 `/` 는 다시 뗀다.
        if scheme != "file" && url.path() == "/" && !Self::has_path(&candidate) {
            text.remove(url[..Position::BeforePath].len());
        }
        Some(LinkUrl(text))
    }

    /// `scheme://host` 뒤에 경로가 적혀 있는지
    fn has_path(candidate: &str) -> bool {
        let Some((_, rest)) = candidate.split_once("://") else {
            return false;
        };
        match rest.find(['/', '\\', '?', '#']) {
            Some(index) => matches!(rest.as_bytes()[index], b'/' | b'\\'),
            None => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkGroup {
    pub id: Id,
    pub name: String,
    pub color: GroupColor,
    pub tags: Vec<String>,
    pub links: Vec<SavedLink>,
    /// 이 그룹을 열 브라우저. 예전 데이터처럼 값이 없으면 기본(Chrome)
    pub browser: Browser,
    /// 카드 왼쪽 아이콘. 값이 없으면(1.3.0 이전 데이터) 기본 폴더
    pub icon: GroupIcon,
    /// 켜 두면 그룹 · 그룹 안 페이지를 항상 시크릿 창(Edge 는 InPrivate)으로 연다. 값이 없으면(1.5.4 이전 데이터) 끔
    pub always_private: bool,
    /// 들어 있는 섹션. 없으면 섹션 밖. 원본 LinkGroups 1.5.5 와 같은 키(`sectionID`)로 적는다
    #[serde(rename = "sectionID", skip_serializing_if = "Option::is_none")]
    pub section_id: Option<Id>,
}

impl LinkGroup {
    /// 그룹을 열 때 열 페이지만 담은 그룹 (스위치를 끈 페이지는 뺀다)
    pub fn for_opening(&self) -> LinkGroup {
        let mut group = self.clone();
        group.links.retain(|link| link.opens_with_group);
        group
    }

    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Id::new(),
            name: name.into(),
            color: GroupColor::Blue,
            tags: Vec::new(),
            links: Vec::new(),
            browser: Browser::DEFAULT,
            icon: GroupIcon::default(),
            always_private: false,
            section_id: None,
        }
    }

    pub fn with_color(mut self, color: GroupColor) -> Self {
        self.color = color;
        self
    }

    pub fn with_tags<S: AsRef<str>>(mut self, tags: &[S]) -> Self {
        self.tags = Self::normalized_tags(tags);
        self
    }

    pub fn with_links(mut self, links: Vec<SavedLink>) -> Self {
        self.links = links;
        self
    }

    pub fn with_browser(mut self, browser: Browser) -> Self {
        self.browser = browser;
        self
    }

    /// 공백 제거 · 앞의 # 제거 · 빈 값과 중복(대소문자 무시) 제거
    pub fn normalized_tags<S: AsRef<str>>(tags: &[S]) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for raw in tags {
            let tag = raw.as_ref().trim().trim_start_matches('#').trim();
            if tag.is_empty() {
                continue;
            }
            if seen.insert(tag.to_lowercase()) {
                result.push(tag.to_owned());
            }
        }
        result
    }

    /// "a, b #c" 처럼 쉼표 · 공백으로 나눈 입력을 태그 목록으로 바꾼다.
    pub fn parse_tags(text: &str) -> Vec<String> {
        let parts: Vec<&str> =
            text.split(|c: char| c == ',' || c.is_whitespace()).filter(|part| !part.is_empty()).collect();
        Self::normalized_tags(&parts)
    }
}

/// browser 가 없거나(1.2.1 이전 데이터) 모르는 값이면 기본 브라우저로, icon 이 없거나 읽을 수 없으면 기본 폴더로 읽는다.
impl<'de> Deserialize<'de> for LinkGroup {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Raw {
            id: Id,
            name: String,
            color: GroupColor,
            tags: Vec<String>,
            links: Vec<SavedLink>,
            #[serde(default)]
            browser: Option<String>,
            #[serde(default)]
            icon: Option<serde_json::Value>,
            #[serde(default)]
            always_private: Option<serde_json::Value>,
            #[serde(default, rename = "sectionID")]
            section_id: Option<serde_json::Value>,
        }

        let raw = Raw::deserialize(deserializer)?;
        Ok(LinkGroup {
            id: raw.id,
            name: raw.name,
            color: raw.color,
            tags: raw.tags,
            links: raw.links,
            browser: raw.browser.as_deref().and_then(Browser::from_raw_value).unwrap_or(Browser::DEFAULT),
            icon: raw.icon.and_then(|value| serde_json::from_value(value).ok()).unwrap_or_default(),
            always_private: raw.always_private.and_then(|value| value.as_bool()).unwrap_or(false),
            section_id: raw.section_id.as_ref().and_then(serde_json::Value::as_str).and_then(Id::parse),
        })
    }
}

/// 그룹을 묶어 접고 펼치는 섹션 (섹션 > 그룹 > 페이지). 원본 LinkGroups 1.5.5 와 같은 형식이다.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    pub is_collapsed: bool,
}

impl Section {
    pub fn new(name: impl Into<String>) -> Self {
        Self { id: Id::new(), name: name.into(), is_collapsed: false }
    }
}

/// 그룹 목록 맨 위 단계의 한 칸: 섹션이나, 섹션에 들지 않은 그룹.
/// 파일에는 원본 LinkGroups 1.5.5 처럼 `"section:<id>"` · `"group:<id>"` 글자로 적는다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SidebarItem {
    Section(Id),
    Group(Id),
}

impl SidebarItem {
    pub fn id(self) -> Id {
        match self {
            SidebarItem::Section(id) | SidebarItem::Group(id) => id,
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let (kind, id) = text.split_once(':')?;
        let id = Id::parse(id.trim())?;
        match kind.trim() {
            "section" => Some(SidebarItem::Section(id)),
            "group" => Some(SidebarItem::Group(id)),
            _ => None,
        }
    }
}

impl std::fmt::Display for SidebarItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SidebarItem::Section(id) => write!(f, "section:{id}"),
            SidebarItem::Group(id) => write!(f, "group:{id}"),
        }
    }
}

impl Serialize for SidebarItem {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// 읽을 수 없는 칸은 건너뛴다 (순서만 조금 달라질 뿐 그룹은 그대로 읽힌다).
fn lenient_sidebar_order<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<SidebarItem>, D::Error> {
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
    Ok(values.iter().filter_map(serde_json::Value::as_str).filter_map(SidebarItem::parse).collect())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkGroupsDocument {
    pub version: i64,
    /// 화면에 보이는 순서 (섹션 안 그룹은 섹션 자리에 이어서)
    pub groups: Vec<LinkGroup>,
    /// 1.4.0 이전 파일에는 없다 (빈 보관함으로 읽는다). 버전 번호는 그대로 두어 이전 앱도 그룹은 읽을 수 있게 한다.
    #[serde(default)]
    pub archive: LinkArchive,
    /// 섹션 (1.5.5). 없으면 적지 않아, 섹션을 쓰지 않는 파일은 이전 형식 그대로다
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<Section>,
    /// 맨 위 단계의 순서 (섹션과 섹션 밖 그룹). 섹션 안 그룹의 순서는 groups 의 순서를 따른다
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "lenient_sidebar_order")]
    pub sidebar_order: Vec<SidebarItem>,
}

impl LinkGroupsDocument {
    pub const CURRENT_VERSION: i64 = 1;

    pub fn new(groups: Vec<LinkGroup>, archive: LinkArchive) -> Self {
        Self { version: Self::CURRENT_VERSION, groups, archive, sections: Vec::new(), sidebar_order: Vec::new() }
    }
}
