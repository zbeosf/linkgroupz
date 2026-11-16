//! 그룹 데이터(data.json) 자동 백업.
//!
//! 하루에 한 번, 그날 처음 확인할 때의 data.json 을 데이터 폴더의 `backups/data-<날짜>.json` 으로 복사해 둔다.
//! 그날 고치기 전의 상태가 남으므로, 잘못 지우거나 바꾼 것을 전날 상태로 되돌릴 수 있다.
//! 내 이미지로 바꾼 그룹 아이콘도 내보내기와 같은 방식(`iconImages` 칸)으로 백업 파일에 함께 넣고, 되돌릴 때 되살린다.
//!
//! 보관 기간이 지난 백업은 지운다 (무제한으로 두면 지우지 않는다). 지우는 것은 이 모듈이 만든 이름(`data-YYYY-MM-DD.json`)의 파일뿐이고,
//! 되돌리기 직전에 따로 남기는 `data-before-restore-…json` 이나 사용자가 넣어 둔 다른 파일은 건드리지 않는다.

use crate::fsutil::write_atomic;
use crate::models::LinkGroupsDocument;
use crate::transfer;
use chrono::{Duration, NaiveDate};
use serde::Serialize;
use std::cmp::Reverse;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 보관 기간 '무제한': 백업을 만들기만 하고 지우지 않는다
pub const KEEP_ALL_BACKUPS: u32 = u32::MAX;
/// 보관 기간으로 고를 수 있는 날 수 (0 은 백업하지 않음, KEEP_ALL_BACKUPS 는 무제한)
pub const BACKUP_DAY_CHOICES: [u32; 6] = [0, 3, 7, 14, 30, KEEP_ALL_BACKUPS];
pub const DEFAULT_BACKUP_DAYS: u32 = 7;

const PREFIX: &str = "data-";
const SUFFIX: &str = ".json";

/// 백업 하나
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    /// "2026-11-12"
    pub date: String,
    #[serde(skip)]
    pub path: PathBuf,
    /// 그룹 수 (읽지 못하면 None)
    pub group_count: Option<usize>,
    pub size: u64,
}

pub struct Backups;

impl Backups {
    /// 이 컴퓨터 시각의 오늘
    pub fn today() -> NaiveDate {
        chrono::Local::now().date_naive()
    }

    /// 되돌리기 전 파일 이름에 붙이는 시각 ("20261112-153000")
    pub fn now_stamp() -> String {
        chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
    }

    /// 오늘 백업을 만들고 보관 기간이 지난 백업을 지운다 (keep_days 가 0 이면 아무것도 하지 않는다).
    /// app_data_directory 는 그룹 아이콘 이미지(icons/)가 있는 앱 데이터 폴더.
    pub fn run(data_file: &Path, directory: &Path, keep_days: u32, app_data_directory: &Path) {
        if keep_days == 0 {
            return;
        }
        let today = Self::today();
        if let Err(error) = Self::back_up_today(data_file, directory, today, app_data_directory) {
            eprintln!("백업하지 못했습니다: {error}");
        }
        Self::prune(directory, keep_days, today);
    }

    /// 데이터 폴더 안의 백업 폴더
    pub fn directory(data_directory: &Path) -> PathBuf {
        data_directory.join("backups")
    }

    /// `data-2026-11-12.json` → 2026-11-12. 이 모양이 아니면 None (그런 파일은 지우지 않는다).
    pub fn date_of(file_name: &str) -> Option<NaiveDate> {
        let date = file_name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
        // 날짜 글자 그대로여야 한다 (앞뒤에 다른 글자가 붙은 이름은 이 모듈이 만든 것이 아니다).
        (date.len() == 10).then_some(())?;
        NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
    }

    fn file_name(date: NaiveDate) -> String {
        format!("{PREFIX}{}{SUFFIX}", date.format("%Y-%m-%d"))
    }

    /// 오늘 백업이 아직 없으면 data.json 을 (그룹 아이콘 이미지와 함께) 남겨 둔다. 만들었으면 그 파일을 돌려준다.
    pub fn back_up_today(data_file: &Path, directory: &Path, today: NaiveDate, app_data_directory: &Path) -> io::Result<Option<PathBuf>> {
        let target = directory.join(Self::file_name(today));
        if target.exists() || !data_file.is_file() {
            return Ok(None);
        }
        let data = fs::read(data_file)?;
        // 읽을 수 없는 파일(쓰는 도중 등)은 백업하지 않는다. 다음 확인 때 다시 한다.
        if serde_json::from_slice::<LinkGroupsDocument>(&data).is_err() {
            return Ok(None);
        }
        fs::create_dir_all(directory)?;
        write_atomic(&target, &transfer::with_icon_images(&data, app_data_directory))?;
        Ok(Some(target))
    }

    /// 보관 기간(keep_days 일, 오늘 포함)이 지나 지울 백업. 이 모듈이 만든 이름의 파일만 고른다. 오래된 것이 먼저.
    /// 백업을 껐거나(0) 무제한이면 없다.
    pub fn expired(directory: &Path, keep_days: u32, today: NaiveDate) -> Vec<PathBuf> {
        if keep_days == 0 || keep_days == KEEP_ALL_BACKUPS {
            return Vec::new();
        }
        // 날짜 계산이 넘치는 아주 긴 기간도 무제한으로 본다.
        let Some(oldest_kept) = today.checked_sub_signed(Duration::days(i64::from(keep_days) - 1)) else {
            return Vec::new();
        };
        let mut expired: Vec<(NaiveDate, PathBuf)> = Self::dated_files(directory).into_iter().filter(|(date, _)| *date < oldest_kept).collect();
        expired.sort();
        expired.into_iter().map(|(_, path)| path).collect()
    }

    /// 보관 기간이 지난 백업을 지운다. 지운 파일을 돌려준다.
    pub fn prune(directory: &Path, keep_days: u32, today: NaiveDate) -> Vec<PathBuf> {
        Self::expired(directory, keep_days, today).into_iter().filter(|path| fs::remove_file(path).is_ok()).collect()
    }

    /// 백업 목록 (최근 것이 먼저)
    pub fn list(directory: &Path) -> Vec<BackupInfo> {
        let mut files = Self::dated_files(directory);
        files.sort_by_key(|(date, _)| Reverse(*date));
        files
            .into_iter()
            .map(|(date, path)| {
                let data = fs::read(&path).ok();
                BackupInfo {
                    date: date.format("%Y-%m-%d").to_string(),
                    group_count: data
                        .as_deref()
                        .and_then(|data| serde_json::from_slice::<LinkGroupsDocument>(data).ok())
                        .map(|document| document.groups.len()),
                    size: data.map_or(0, |data| data.len() as u64),
                    path,
                }
            })
            .collect()
    }

    fn dated_files(directory: &Path) -> Vec<(NaiveDate, PathBuf)> {
        let Ok(entries) = fs::read_dir(directory) else {
            return Vec::new();
        };
        entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .filter_map(|path| Some((Self::date_of(path.file_name()?.to_str()?)?, path)))
            .collect()
    }

    /// 백업을 data.json 으로 되돌리고, 백업에 든 그룹 아이콘 이미지를 되살린다.
    /// 지금 data.json 은 (아이콘 이미지와 함께) `data-before-restore-<시각>.json` 으로 남긴다 (자동으로 지우지 않는다).
    pub fn restore(backup: &Path, data_file: &Path, directory: &Path, now_stamp: &str, app_data_directory: &Path) -> io::Result<()> {
        let data = fs::read(backup)?;
        serde_json::from_slice::<LinkGroupsDocument>(&data)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, crate::tr!("백업 파일을 읽지 못했습니다: {}", "Couldn't read the backup file: {}", error)))?;
        if data_file.is_file() {
            fs::create_dir_all(directory)?;
            let current = transfer::with_icon_images(&fs::read(data_file)?, app_data_directory);
            write_atomic(&directory.join(format!("{PREFIX}before-restore-{now_stamp}{SUFFIX}")), &current)?;
        }
        transfer::restore_icon_images(&data, app_data_directory);
        // data.json 에는 이미지 칸 없이 그룹만 적는다 (앱이 저장할 때와 같은 모양).
        let plain = serde_json::from_slice::<serde_json::Value>(&data)
            .ok()
            .and_then(|mut value| {
                value.as_object_mut()?.remove("iconImages");
                serde_json::to_vec_pretty(&value).ok()
            })
            .unwrap_or(data);
        write_atomic(data_file, &plain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn names() {
        assert_eq!(Backups::date_of("data-2026-11-12.json"), Some(day("2026-11-12")));
        for other in ["data-before-restore-20261112-101010.json", "data-2026-11-12.json.bak", "data-2026-1-2.json", "x-2026-11-12.json", "data-2026-13-01.json", "data.json"] {
            assert_eq!(Backups::date_of(other), None, "{other} 는 백업 이름이 아니다");
        }
    }

    #[test]
    fn back_up_prune_restore() {
        let folder = tempfile::tempdir().unwrap();
        let data_file = folder.path().join("data.json");
        let backups = Backups::directory(folder.path());
        let document = |name: &str| format!(r#"{{"version":1,"groups":[{{"id":"{}","name":"{name}","color":"blue","tags":[],"links":[]}}]}}"#, crate::ids::Id::new());

        assert_eq!(Backups::back_up_today(&data_file, &backups, day("2026-11-01"), folder.path()).unwrap(), None, "data.json 이 없으면 백업하지 않는다");
        fs::write(&data_file, document("첫날")).unwrap();
        assert!(Backups::back_up_today(&data_file, &backups, day("2026-11-01"), folder.path()).unwrap().is_some());
        fs::write(&data_file, document("바꾼 뒤")).unwrap();
        assert_eq!(Backups::back_up_today(&data_file, &backups, day("2026-11-01"), folder.path()).unwrap(), None, "하루에 한 번만 (그날 처음 상태가 남는다)");
        fs::write(&data_file, "쓰는 도중").unwrap();
        assert_eq!(Backups::back_up_today(&data_file, &backups, day("2026-11-02"), folder.path()).unwrap(), None, "읽을 수 없는 파일은 백업하지 않는다");
        fs::write(&data_file, document("둘째 날")).unwrap();
        for date in ["2026-11-02", "2026-11-05", "2026-11-08"] {
            Backups::back_up_today(&data_file, &backups, day(date), folder.path()).unwrap();
        }
        // 이 모듈이 만들지 않은 파일
        fs::write(backups.join("data-before-restore-20261101-090000.json"), "{}").unwrap();
        fs::write(backups.join("메모.txt"), "x").unwrap();

        let dates = |list: Vec<BackupInfo>| list.into_iter().map(|info| info.date).collect::<Vec<_>>();
        assert_eq!(dates(Backups::list(&backups)), ["2026-11-08", "2026-11-05", "2026-11-02", "2026-11-01"], "최근 것이 먼저, 백업 이름만");
        assert_eq!(Backups::list(&backups)[3].group_count, Some(1));

        // 7일 보관: 11-08 기준으로 11-02 부터 남는다
        let expired = Backups::expired(&backups, 7, day("2026-11-08"));
        assert_eq!(expired, [backups.join("data-2026-11-01.json")]);
        assert!(Backups::expired(&backups, 0, day("2026-12-31")).is_empty(), "백업을 끄면 지우지도 않는다");
        assert!(Backups::expired(&backups, KEEP_ALL_BACKUPS, day("2099-12-31")).is_empty(), "무제한이면 지우지 않는다");
        assert!(Backups::expired(&backups, u32::MAX - 1, day("2099-12-31")).is_empty(), "날짜 계산이 넘치는 기간도 지우지 않는다");
        Backups::run(&data_file, &backups, KEEP_ALL_BACKUPS, folder.path());
        assert_eq!(Backups::list(&backups).len(), 5, "무제한: 오늘 백업을 더하고(4 → 5) 지난 것은 지우지 않는다");
        fs::remove_file(backups.join(format!("data-{}.json", Backups::today().format("%Y-%m-%d")))).unwrap();
        assert_eq!(Backups::prune(&backups, 7, day("2026-11-08")), expired);
        assert!(backups.join("data-before-restore-20261101-090000.json").exists() && backups.join("메모.txt").exists(), "다른 파일은 그대로");
        assert_eq!(Backups::prune(&backups, 3, day("2026-11-08")).len(), 2, "3일 보관: 11-06 부터");
        assert_eq!(dates(Backups::list(&backups)), ["2026-11-08"]);

        // 되돌리기: 지금 파일을 따로 남기고 백업 내용으로 바꾼다
        fs::write(&data_file, document("지금")).unwrap();
        Backups::restore(&backups.join("data-2026-11-08.json"), &data_file, &backups, "20261108-120000", folder.path()).unwrap();
        assert!(fs::read_to_string(&data_file).unwrap().contains("둘째 날"));
        assert!(fs::read_to_string(backups.join("data-before-restore-20261108-120000.json")).unwrap().contains("지금"));
        fs::write(backups.join("data-2026-11-07.json"), "깨진 백업").unwrap();
        assert!(Backups::restore(&backups.join("data-2026-11-07.json"), &data_file, &backups, "x", folder.path()).is_err(), "읽을 수 없는 백업은 되돌리지 않는다");
        assert!(fs::read_to_string(&data_file).unwrap().contains("둘째 날"), "실패하면 그대로");
    }

    #[test]
    fn icon_images_are_backed_up_and_restored() {
        use crate::group_icon::GroupIconStore;
        let folder = tempfile::tempdir().unwrap();
        let data_file = folder.path().join("data.json");
        let backups = Backups::directory(folder.path());
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::new(1, 1).write_to(&mut png, image::ImageFormat::Png).unwrap();
        fs::create_dir_all(GroupIconStore::directory(folder.path())).unwrap();
        let icon = GroupIconStore::file_path("mine.png", folder.path()).unwrap();
        fs::write(&icon, png.get_ref()).unwrap();
        fs::write(&data_file, format!(r#"{{"version":1,"groups":[{{"id":"{}","name":"내 아이콘","color":"blue","tags":[],"links":[],"icon":{{"image":"mine.png"}}}}]}}"#, crate::ids::Id::new())).unwrap();

        let backup = Backups::back_up_today(&data_file, &backups, day("2026-11-01"), folder.path()).unwrap().unwrap();
        assert!(fs::read_to_string(&backup).unwrap().contains("iconImages"), "백업에 아이콘 이미지가 들어간다");
        assert_eq!(Backups::list(&backups)[0].group_count, Some(1), "이미지 칸이 있어도 백업 목록에 그룹 수가 나온다");

        // 아이콘 파일이 지워진 뒤 되돌리면 이미지도 돌아온다
        fs::remove_file(&icon).unwrap();
        Backups::restore(&backup, &data_file, &backups, "20261101-120000", folder.path()).unwrap();
        assert_eq!(fs::read(&icon).unwrap(), png.into_inner());
        assert!(!fs::read_to_string(&data_file).unwrap().contains("iconImages"), "data.json 에는 이미지 칸을 적지 않는다");
    }
}
