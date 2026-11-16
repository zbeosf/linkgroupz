//! 화면 · 알림 글의 언어 (한국어 · 영어).
//!
//! 앱 전체가 하나의 언어를 쓴다. 앱이 켜질 때와 설정에서 바꿀 때 `set_language` 로 정하고,
//! 글을 만드는 곳은 `tr!("한국어", "English")` 처럼 두 말을 함께 적는다 (format! 처럼 인자도 받는다).
//! 화면(웹 페이지)의 글은 crates/app/ui/lib/i18n.js 가 맡는다.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Ko,
    En,
}

impl Language {
    /// 시스템 언어 이름(ko-KR, en-US …)으로 고른다. 한국어가 아니면 영어.
    pub fn from_locale(locale: &str) -> Self {
        if locale.to_ascii_lowercase().starts_with("ko") {
            Self::Ko
        } else {
            Self::En
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Ko => "ko",
            Self::En => "en",
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set_language(language: Language) {
    CURRENT.store(language as u8, Ordering::Relaxed);
}

pub fn language() -> Language {
    if CURRENT.load(Ordering::Relaxed) == Language::En as u8 {
        Language::En
    } else {
        Language::Ko
    }
}

pub fn is_english() -> bool {
    language() == Language::En
}

/// 지금 언어로 고른 글. `tr!("한국어", "English")` 는 &str, 인자가 있으면 String.
#[macro_export]
macro_rules! tr {
    ($ko:literal, $en:literal $(,)?) => {
        if $crate::i18n::is_english() { $en } else { $ko }
    };
    ($ko:literal, $en:literal, $($arg:tt)*) => {
        if $crate::i18n::is_english() { format!($en, $($arg)*) } else { format!($ko, $($arg)*) }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_language() {
        assert_eq!(Language::from_locale("ko-KR"), Language::Ko);
        assert_eq!(Language::from_locale("en-US"), Language::En);
        assert_eq!(Language::from_locale("ja-JP"), Language::En, "한국어가 아니면 영어");
        set_language(Language::En);
        assert_eq!(tr!("그룹", "Group"), "Group");
        assert_eq!(tr!("그룹 {}개", "{} groups", 3), "3 groups");
        set_language(Language::Ko);
        assert_eq!(tr!("그룹 {}개", "{} groups", 3), "그룹 3개");
    }
}
