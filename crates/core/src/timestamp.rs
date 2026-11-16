use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 시각.
///
/// Swift 의 `Date` 처럼 2001-01-01 UTC 부터 지난 초로 들고 있고, 파일 · 키체인에도 그 숫자를 그대로 적는다.
/// Swift 판 LinkGroups 가 쓴 data.json 의 `archivedAt`, 키체인 쿠키의 `expires` 와 같은 형식이고,
/// 적었다가 다시 읽어도 값이 한 비트도 달라지지 않는다.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(f64);

/// 유닉스 기준 시각(1970)과 Swift 기준 시각(2001)의 차이(초)
const REFERENCE_DATE_OFFSET: f64 = 978_307_200.0;

impl Timestamp {
    pub fn now() -> Self {
        Self::from(SystemTime::now())
    }

    pub fn from_unix_seconds(seconds: f64) -> Self {
        Self(seconds - REFERENCE_DATE_OFFSET)
    }

    pub fn from_unix_millis(millis: f64) -> Self {
        Self::from_unix_seconds(millis / 1000.0)
    }

    pub fn unix_seconds(self) -> f64 {
        self.0 + REFERENCE_DATE_OFFSET
    }

    pub fn plus_seconds(self, seconds: f64) -> Self {
        Self(self.0 + seconds)
    }
}

impl From<SystemTime> for Timestamp {
    fn from(time: SystemTime) -> Self {
        match time.duration_since(UNIX_EPOCH) {
            Ok(elapsed) => Self::from_unix_seconds(elapsed.as_secs_f64()),
            Err(before) => Self::from_unix_seconds(-before.duration().as_secs_f64()),
        }
    }
}

impl From<Timestamp> for SystemTime {
    fn from(time: Timestamp) -> Self {
        let seconds = time.unix_seconds();
        if seconds >= 0.0 {
            UNIX_EPOCH + Duration::from_secs_f64(seconds)
        } else {
            UNIX_EPOCH - Duration::from_secs_f64(-seconds)
        }
    }
}
