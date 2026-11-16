//! LinkGroupz 의 화면과 무관한 부분: 모델 · JSON 저장소 · 검색 · 브라우저(Chrome · Edge · Whale) 실행 · 확장 감지 ·
//! 탭 스냅샷 · 키체인 쿠키 · 사이트 아이콘 주소 · 그룹 아이콘 이미지 · 보관함 · 업데이트 규칙.

pub mod archive;
pub mod base64url;
pub mod backups;
pub mod brand;
pub mod browser;
pub mod cookies;
pub mod drag;
pub mod extension;
pub mod favicon;
mod fsutil;
pub mod group_icon;
pub mod group_sync;
pub mod i18n;
pub mod ids;
pub mod launcher;
pub mod leveldb;
pub mod models;
pub mod native;
pub mod platform;
pub mod profiles;
pub mod release_notes;
pub mod saved_tab_groups;
pub mod search;
pub mod sections;
pub mod secret;
pub mod session_file;
pub mod snapshot;
pub mod store;
pub mod tabs;
pub mod timestamp;
pub mod transfer;
pub mod update;

pub use archive::{ArchivedGroup, ArchivedLink, LinkArchive};
pub use base64url::Base64Url;
pub use browser::Browser;
pub use cookies::{CookieVault, CookieVaultError, StoredCookie};
pub use drag::LinkDragPayload;
pub use extension::{BundledExtension, ChromeExtensionDetector, FoundExtension, GroupOpenMode};
pub use favicon::FaviconParser;
pub use fsutil::write_atomic;
pub use group_icon::{GroupIcon, GroupIconStore, GroupIconStoreError};
pub use ids::Id;
pub use launcher::{ChromeLauncher, LauncherError, MarkerPayload, TabGroupMarker};
pub use models::{GroupColor, LinkGroup, LinkGroupsDocument, LinkUrl, SavedLink, Section, SidebarItem};
pub use sections::DropTarget;
pub use native::{NativeMessaging, NativeSnapshotError, NativeSnapshotStore};
pub use release_notes::{ReleaseEntry, ReleaseNotes};
pub use search::{SearchFilter, SearchScope};
pub use secret::{SecretError, SecretStore};
pub use snapshot::{
    ChromeSnapshot, ChromeTabGroupImport, ChromeTabSource, PlanAddition, TabGroupImportPlan, TabSourceResult,
};
pub use store::{move_elements, LinkStore, MoveResult};
pub use tabs::{
    CandidateStatus, ChromeTab, ChromeTabGroupInfo, ChromeTabsReader, DirectTabsHost, ScriptFailure, ScriptValue,
    TabImportCandidate, TabsError, TabsHost,
};
pub use timestamp::Timestamp;
pub use update::{AppVersion, UpdatePromptPolicy, UpdateSource};
