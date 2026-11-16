//! 릴리스 묶음을 만들고(서명 포함) GitHub 릴리스로 올린다. macOS 에서 실행한다.
//!
//! ```text
//! cargo xtask release [--windows <setup.exe>] [--mac-only] [--publish]
//! ```
//!
//! 1. macOS 앱을 빌드한다 (DMG 와, 업데이터가 받는 `.app.tar.gz`).
//! 2. Windows 설치 파일을 가져온다: 이 저장소의 GitHub Actions(.github/workflows/windows.yml)가
//!    태그 `v<버전>` 에서 만든 것을 내려받거나, `--windows` 로 준 파일을 쓴다.
//! 3. 업데이터가 받는 파일마다 서명한다. 서명 키는 이 컴퓨터에만 있고 GitHub 에는 올리지 않는다.
//! 4. 앱이 새 버전을 확인할 때 읽는 `latest.json` 을 쓴다.
//! 5. `--publish` 를 주면 릴리스 저장소(공개, update-source.env 의 LINKGROUPZ_RELEASES_REPO)에 릴리스 `v<버전>` 으로 올린다. 주지 않으면 dist/release-<버전>/ 에 모아만 둔다.

use crate::dist::{self, host_triple, tauri_cli};
use crate::{execute, has_option, licenses, option_value, workspace_root, Outcome};
use linkgroupz_core::{AppVersion, ReleaseNotes, UpdateSource};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 서명 키 파일. 환경 변수 TAURI_SIGNING_PRIVATE_KEY_PATH 로 바꿀 수 있다.
const DEFAULT_KEY_FILE: &str = ".tauri/linkgroupz-updater.key";
/// Windows 워크플로 파일 이름과, 그 워크플로가 올리는 아티팩트 이름
const WINDOWS_WORKFLOW: &str = "windows.yml";
const WINDOWS_ARTIFACT: &str = "windows-setup";

fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// 릴리스 저장소와 공개 키. 앱을 빌드할 때 들어가는 것과 같은 곳(환경 변수 · update-source.env)에서 읽는다.
fn update_source(root: &Path) -> Outcome<UpdateSource> {
    UpdateSource::load(root).ok_or_else(|| {
        format!("릴리스 저장소와 공개 키가 없습니다. update-source.env.example 을 {} 로 복사해 값을 채워 주세요.", UpdateSource::FILE_NAME).into()
    })
}

fn key_file() -> Outcome<PathBuf> {
    let file = match std::env::var_os("TAURI_SIGNING_PRIVATE_KEY_PATH") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(std::env::var_os("HOME").ok_or("HOME 이 없습니다")?).join(DEFAULT_KEY_FILE),
    };
    if !file.is_file() {
        return Err(format!("서명 키가 없습니다: {}", file.display()).into());
    }
    Ok(file)
}

pub fn run(options: &[String]) -> Outcome {
    if !cfg!(target_os = "macos") {
        return Err("release 는 macOS 에서 실행합니다 (Windows 설치 파일은 GitHub Actions 가 만든다)".into());
    }
    let root = workspace_root();
    let version = version();
    let tag = format!("v{version}");
    let source = update_source(&root)?;
    let repository = source.repository();
    let key = key_file()?;
    let (notes_ko, notes_en) = release_notes(version)?;
    // 앱의 업데이트 창은 ENGLISH_NOTES 줄로 나눠 화면 언어 쪽을 보여 준다 (crates/app/ui/update.js).
    let notes = format!("{notes_ko}\n\n{ENGLISH_NOTES}\n{notes_en}");
    licenses::run(&["--check".to_owned()])?;

    let out = root.join("dist").join(format!("release-{version}"));
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out)?;
    let download_url = |file_name: &str| source.download_url(&tag, file_name);
    let mut platforms = Map::new();

    // macOS
    dist::run(&[])?;
    let dmg = format!("LinkGroupz-{version}.dmg");
    fs::copy(root.join("dist").join(&dmg), out.join(&dmg))?;
    let triple = host_triple()?;
    let arch = triple.split('-').next().unwrap_or_default().to_owned();
    let archive = format!("LinkGroupz-{version}-darwin-{arch}.app.tar.gz");
    execute(
        Command::new("tar")
            // 리소스 포크(._ 파일)를 따로 넣지 않는다.
            .env("COPYFILE_DISABLE", "1")
            // 묶음에 이 컴퓨터의 사용자 · 그룹 이름을 적지 않는다.
            .args(["--uid", "0", "--gid", "0", "--uname", "", "--gname", ""])
            .arg("-czf")
            .arg(out.join(&archive))
            .arg("-C")
            .arg(root.join("target/release/bundle/macos"))
            .arg("LinkGroupz.app"),
    )?;
    platforms.insert(format!("darwin-{arch}"), json!({ "signature": sign(&out.join(&archive), &key, version)?, "url": download_url(&archive) }));

    // Windows
    let setup = format!("LinkGroupz-{version}-setup.exe");
    if !has_option(options, "--mac-only") {
        match option_value(options, "--windows") {
            Some(path) => {
                fs::copy(&path, out.join(&setup)).map_err(|error| format!("{path} 을 읽지 못했습니다: {error}"))?;
            }
            None => download_windows_setup(&root, &tag, &out, &setup)?,
        }
        platforms.insert("windows-x86_64".to_owned(), json!({ "signature": sign(&out.join(&setup), &key, version)?, "url": download_url(&setup) }));
    }

    let manifest = json!({
        "version": version,
        "notes": notes,
        "pub_date": now_rfc3339()?,
        "platforms": platforms,
    });
    fs::write(out.join("latest.json"), serde_json::to_string_pretty(&manifest)? + "\n")?;

    // 올릴 파일: 서명(.sig)은 latest.json 에 들어 있어 따로 올리지 않는다.
    let mut assets: Vec<PathBuf> = fs::read_dir(&out)?
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.extension().is_none_or(|extension| extension != "sig"))
        .collect();
    assets.sort();
    println!("\n릴리스 {tag} 묶음: {}", out.display());
    for asset in &assets {
        println!("  {}", asset.file_name().unwrap_or_default().to_string_lossy());
    }

    if has_option(options, "--publish") {
        // 릴리스 페이지에는 받을 파일 · 처음 여는 방법을 덧붙인다 (앱이 보여 주는 바뀐 점에는 넣지 않는다).
        // 'Source code' 두 개는 GitHub 가 모든 릴리스에 자동으로 붙인다 (이 버전의 소스). 앱을 쓰려면 받지 않아도 된다고 적는다.
        let notes_file = root.join("target").join("release-notes.md");
        let (guide_ko, guide_en) = release_page_guide(&dmg, &setup);
        let page = format!("### 바뀐 점\n{notes_ko}\n\n{guide_ko}\n\n---\n\n### What's new\n{notes_en}\n\n{guide_en}");
        fs::write(&notes_file, page)?;
        execute(
            Command::new("gh")
                .args(["release", "create", &tag, "--repo", repository, "--title", &format!("LinkGroupz {version}"), "--notes-file"])
                .arg(&notes_file)
                .args(&assets),
        )?;
        println!("올렸습니다: {}/tag/{tag}", source.releases_page_url());
    } else {
        println!("올리지 않았습니다. 올리려면 `cargo xtask release --publish` ({repository} 의 릴리스 {tag}).");
    }
    Ok(())
}

/// 이 버전의 바뀐 점 (crates/core/src/release_notes.rs). 없으면 릴리스를 만들지 않는다.
/// (한국어 목록, 영어 목록)
fn release_notes(version: &str) -> Outcome<(String, String)> {
    let current = AppVersion::parse(version).ok_or("버전을 읽지 못했습니다")?;
    let entry = ReleaseNotes::entries()
        .iter()
        .find(|entry| entry.version == current)
        .ok_or_else(|| format!("{version} 의 바뀐 점이 없습니다. crates/core/src/release_notes.rs 맨 위에 더해 주세요."))?;
    let list = |changes: &[String]| changes.iter().map(|change| format!("- {change}")).collect::<Vec<_>>().join("\n");
    if entry.changes_en.is_empty() {
        return Err(format!("{version} 의 영어 바뀐 점이 없습니다. crates/core/src/release_notes.rs 에 더해 주세요.").into());
    }
    Ok((list(&entry.changes), list(&entry.changes_en)))
}

/// 바뀐 점에서 영어가 시작되는 줄 (crates/app/ui/update.js 와 같게)
const ENGLISH_NOTES: &str = "### English";

/// 릴리스 페이지 아래에 붙이는 안내 (받을 파일 · 처음 여는 방법). 쓰는 사람 말로만 쓴다. (한국어, 영어)
fn release_page_guide(dmg: &str, setup: &str) -> (String, String) {
    let ko = format!(
        "### 받을 파일\n\
         - **macOS**: `{dmg}` — Apple Silicon(M1 이후) Mac 전용입니다. Intel Mac 에서는 실행되지 않습니다.\n\
         - **Windows 10 · 11 (64비트)**: `{setup}`\n\
         \n\
         다른 파일은 받지 않아도 됩니다. `…app.tar.gz` 와 `latest.json` 은 앱이 스스로 업데이트할 때 쓰는 파일이고, \
         맨 아래 `Source code` 두 개는 이 버전의 소스 코드입니다.\n\
         \n\
         ### 처음 열 때\n\
         - **macOS**: DMG 를 열어 LinkGroupz 를 Applications 로 옮긴 뒤 엽니다. \"'LinkGroupz'을(를) 열지 않음\" 경고가 뜨면 **완료** 를 누르고, \
         **시스템 설정 → 개인정보 보호 및 보안 → 그래도 열기** 를 한 번 누릅니다.\n\
         - **Windows**: 설치 파일을 실행합니다. **Windows 의 PC 보호** 창이 뜨면 **추가 정보 → 실행** 을 누릅니다.\n\
         \n\
         설치한 뒤에는 앱이 새 버전을 알려 줍니다."
    );
    let en = format!(
        "### Downloads\n\
         - **macOS**: `{dmg}` — for Apple Silicon (M1 or later) Macs only. It doesn't run on Intel Macs.\n\
         - **Windows 10 · 11 (64-bit)**: `{setup}`\n\
         \n\
         You don't need the other files. `…app.tar.gz` and `latest.json` are used by the app to update itself, \
         and the two `Source code` files at the bottom are this version's source code.\n\
         \n\
         ### First launch\n\
         - **macOS**: Open the DMG, move LinkGroupz to Applications and open it. If macOS says it can't open \"LinkGroupz\", click **Done**, \
         then click **Open Anyway** once in **System Settings → Privacy & Security**.\n\
         - **Windows**: Run the installer. If **Windows protected your PC** appears, click **More info → Run anyway**.\n\
         \n\
         Once installed, the app tells you when a new version is out."
    );
    (ko, en)
}

/// 파일에 서명하고 서명 글(`<파일>.sig` 의 내용)을 돌려준다.
/// 서명에 버전을 넣어, 앱이 latest.json 에 적힌 버전과 받은 파일의 버전이 같은지 확인할 수 있게 한다.
fn sign(file: &Path, key: &Path, version: &str) -> Outcome<String> {
    let password = std::env::var("TAURI_SIGNING_PRIVATE_KEY_PASSWORD").unwrap_or_default();
    let output = tauri_cli()
        .args(["signer", "sign", "--private-key-path"])
        .arg(key)
        .args(["--password", &password, "--app-version", version])
        .arg(file)
        // 서명 글이 화면에 찍히지 않게 한다 (파일로도 쓰인다).
        .output()
        .map_err(|error| format!("서명 도구를 실행하지 못했습니다: {error}"))?;
    if !output.status.success() {
        return Err(format!("{} 에 서명하지 못했습니다:\n{}", file.display(), String::from_utf8_lossy(&output.stderr)).into());
    }
    let mut signature_file = file.as_os_str().to_owned();
    signature_file.push(".sig");
    let signature = fs::read_to_string(&signature_file)?.trim().to_owned();
    if signature.is_empty() {
        return Err(format!("{} 의 서명이 비어 있습니다", file.display()).into());
    }
    println!("서명: {}", file.file_name().unwrap_or_default().to_string_lossy());
    Ok(signature)
}

/// GitHub Actions 가 태그 `v<버전>` 에서 만든 Windows 설치 파일을 내려받는다.
fn download_windows_setup(root: &Path, tag: &str, out: &Path, setup: &str) -> Outcome {
    let output = Command::new("gh")
        .current_dir(root)
        .args(["run", "list", "--workflow", WINDOWS_WORKFLOW, "--limit", "50", "--json", "databaseId,headBranch,status,conclusion"])
        .output()
        .map_err(|error| format!("gh 를 실행하지 못했습니다: {error}"))?;
    if !output.status.success() {
        return Err(format!("워크플로 실행 목록을 읽지 못했습니다:\n{}", String::from_utf8_lossy(&output.stderr)).into());
    }
    let runs: Vec<Value> = serde_json::from_slice(&output.stdout)?;
    // 태그로 시작한 실행은 headBranch 가 태그 이름이다. 목록은 최신이 먼저다.
    let run = runs.iter().find(|run| run["headBranch"] == tag).ok_or_else(|| {
        format!("태그 {tag} 의 Windows 빌드가 없습니다. 태그를 올렸는지 확인하거나(`git push origin {tag}`), --windows <파일> 또는 --mac-only 를 주세요.")
    })?;
    if run["conclusion"] != "success" {
        return Err(format!("태그 {tag} 의 Windows 빌드가 아직 끝나지 않았거나 실패했습니다 (status: {}, conclusion: {}).", run["status"], run["conclusion"]).into());
    }
    let folder = root.join("target").join("windows-setup");
    let _ = fs::remove_dir_all(&folder);
    execute(
        Command::new("gh")
            .current_dir(root)
            .args(["run", "download", &run["databaseId"].to_string(), "--name", WINDOWS_ARTIFACT, "--dir"])
            .arg(&folder),
    )?;
    // 태그와 Cargo.toml 의 버전이 어긋났으면 다른 버전의 파일이 들어 있다.
    fs::copy(folder.join(setup), out.join(setup)).map_err(|_| format!("Windows 빌드에 {setup} 이 없습니다 (태그와 Cargo.toml 의 버전이 같은지 확인)"))?;
    Ok(())
}

fn now_rfc3339() -> Outcome<String> {
    let output = Command::new("date").args(["-u", "+%Y-%m-%dT%H:%M:%SZ"]).output()?;
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_guide_is_plain() {
        let (ko, en) = release_page_guide("LinkGroupz-9.9.9.dmg", "LinkGroupz-9.9.9-setup.exe");
        for guide in [&ko, &en] {
            assert!(guide.contains("`LinkGroupz-9.9.9.dmg`") && guide.contains("`LinkGroupz-9.9.9-setup.exe`"));
            for word in ["서명", "토큰", "공증", "minisign", "pubkey", "signature", "token", "notariz"] {
                assert!(!guide.contains(word), "릴리스 페이지 안내에 '{word}' 같은 말은 넣지 않는다");
            }
        }
        if let Ok(path) = std::env::var("LINKGROUPZ_GUIDE_OUT") {
            fs::write(path, format!("{ko}\n\n{en}")).unwrap();
        }
    }
}
