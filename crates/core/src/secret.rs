use crate::platform;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    /// 운영체제 보관소(macOS 키체인 · Windows 자격 증명 관리자)가 돌려준 오류
    #[error("{0}")]
    Store(String),
}

/// 비밀 값 하나를 운영체제 보관소에 암호화해 두고 꺼낸다.
///
/// macOS 는 로그인 키체인의 일반 암호 항목(서비스 · 계정), Windows 는 자격 증명 관리자를 쓴다.
#[derive(Clone, Debug)]
pub struct SecretStore {
    pub service: String,
    pub account: String,
    /// 키체인 접근 앱에 보이는 이름 (macOS)
    pub label: String,
}

impl SecretStore {
    pub fn new(service: impl Into<String>, account: impl Into<String>, label: impl Into<String>) -> Self {
        Self { service: service.into(), account: account.into(), label: label.into() }
    }

    /// 저장된 값. 항목이 없으면 None.
    pub fn load(&self) -> Result<Option<Vec<u8>>, SecretError> {
        platform::secret_load(&self.service, &self.account)
    }

    /// 있으면 바꾸고 없으면 새로 만든다.
    pub fn save(&self, data: &[u8]) -> Result<(), SecretError> {
        platform::secret_save(&self.service, &self.account, &self.label, data)
    }

    /// 항목이 없어도 오류가 아니다.
    pub fn delete(&self) -> Result<(), SecretError> {
        platform::secret_delete(&self.service, &self.account)
    }
}
