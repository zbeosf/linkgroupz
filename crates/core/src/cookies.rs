use crate::brand;
use crate::secret::{SecretError, SecretStore};
use crate::store::sorted_keys;
use crate::timestamp::Timestamp;
use serde::{Deserialize, Serialize};

/// 키체인에 넣을 수 있도록 쿠키를 값만 담은 형태로 바꾼 것.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    /// None 이면 세션 쿠키
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<Timestamp>,
    pub secure: bool,
    pub http_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_site: Option<String>,
}

impl StoredCookie {
    pub fn new(name: impl Into<String>, value: impl Into<String>, domain: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            domain: domain.into(),
            path: "/".to_owned(),
            expires: None,
            secure: false,
            http_only: false,
            same_site: None,
        }
    }

    pub fn with_expires(mut self, expires: Timestamp) -> Self {
        self.expires = Some(expires);
        self
    }

    pub fn is_expired(&self, at: Timestamp) -> bool {
        self.expires.is_some_and(|expires| expires <= at)
    }

    /// 쿠키가 속한 사이트 (도메인 앞의 `.` 을 뗀 것)
    pub fn site(&self) -> &str {
        self.domain.strip_prefix('.').unwrap_or(&self.domain)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CookieVaultError {
    #[error("{}", crate::tr!("키체인 오류: {}", "Keychain error: {}", .0))]
    Keychain(#[from] SecretError),
    #[error("{}", crate::tr!("저장된 쿠키를 읽지 못했습니다: {}", "Couldn't read the saved cookies: {}", .0))]
    Json(#[from] serde_json::Error),
}

/// 미리보기 로그인 쿠키를 로그인 키체인의 항목 하나에 암호화해 보관한다 (macOS).
/// 디스크에는 키체인 말고 아무것도 남기지 않는다.
///
/// Windows 는 자격 증명 관리자 항목 하나의 크기가 작아(2.5KB) 쿠키를 담을 수 없다.
/// 그래서 Windows 에서는 이 보관함 대신 WebView2 의 프로필(브라우저가 직접 암호화해 보관)을 쓴다.
#[derive(Clone, Debug)]
pub struct CookieVault {
    secret: SecretStore,
}

impl Default for CookieVault {
    fn default() -> Self {
        Self::new(brand::COOKIE_VAULT_SERVICE)
    }
}

impl CookieVault {
    pub fn new(service: &str) -> Self {
        Self { secret: SecretStore::new(service, "cookies", crate::tr!("LinkGroupz 미리보기 로그인 쿠키", "LinkGroupz Preview Sign-in Cookies")) }
    }

    /// 저장된 쿠키. 항목이 없으면 빈 목록.
    pub fn load(&self) -> Result<Vec<StoredCookie>, CookieVaultError> {
        match self.secret.load()? {
            Some(data) => Ok(serde_json::from_slice(&data)?),
            None => Ok(Vec::new()),
        }
    }

    /// 만료된 쿠키는 빼고 저장한다. 내용이 이미 같으면 키체인에 쓰지 않는다. 썼으면 true.
    pub fn save(&self, cookies: &[StoredCookie], now: Timestamp) -> Result<bool, CookieVaultError> {
        let mut kept: Vec<&StoredCookie> = cookies.iter().filter(|cookie| !cookie.is_expired(now)).collect();
        kept.sort_by(|a, b| (&a.domain, &a.path, &a.name).cmp(&(&b.domain, &b.path, &b.name)));
        if kept.is_empty() {
            self.clear()?;
            return Ok(true);
        }
        let data = serde_json::to_vec(&sorted_keys(serde_json::to_value(&kept)?))?;
        if self.secret.load()?.is_some_and(|existing| existing == data) {
            return Ok(false);
        }
        self.secret.save(&data)?;
        Ok(true)
    }

    pub fn clear(&self) -> Result<(), CookieVaultError> {
        Ok(self.secret.delete()?)
    }
}
