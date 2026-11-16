use crate::{execute, has_option, option_value, workspace_root, Outcome};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// 앱과 함께 넣는 호스트 실행 파일 이름 (crates/core/src/brand.rs 의 HOST_EXECUTABLE_NAME 과 같아야 한다)
const HOST_NAME: &str = "linkgroupz-host";

pub fn host_triple() -> Outcome<String> {
    let output = Command::new("rustc").arg("-vV").output()?;
    let text = String::from_utf8(output.stdout)?;
    text.lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .ok_or_else(|| "rustc -vV 에서 host 를 찾지 못했습니다".into())
}

/// 배포하는 실행 파일에 이 컴퓨터의 폴더 이름(사용자 이름이 든 홈 폴더 등)이 남지 않게 한다.
///
/// Rust 는 오류 메시지에 쓰려고 소스 파일의 전체 경로를 실행 파일에 넣는다
/// (`/Users/<이름>/.cargo/registry/src/…/lib.rs`). 빌드할 때 경로의 머리를 다른 이름으로 바꿔 적게 한다.
fn hide_local_paths(command: &mut Command) {
    let root = workspace_root();
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let cargo_home = std::env::var_os("CARGO_HOME").map(PathBuf::from).or_else(|| home.as_ref().map(|home| home.join(".cargo")));
    // 뒤에 적은 것이 먼저 맞춰지므로 넓은 것(홈 폴더)부터 좁은 것(작업 공간) 순으로 적는다.
    let mut flags: Vec<String> = Vec::new();
    if let Some(home) = &home {
        flags.push(format!("--remap-path-prefix={}=home", home.display()));
    }
    if let Some(cargo_home) = &cargo_home {
        flags.push(format!("--remap-path-prefix={}=cargo", cargo_home.display()));
    }
    flags.push(format!("--remap-path-prefix={}=linkgroupz", root.display()));
    // 경로에 공백이 있어도 되게 칸을 0x1f 로 나누는 형식을 쓴다. 이미 준 값이 있으면 그 뒤에 붙인다.
    let mut encoded = std::env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    for flag in flags {
        if !encoded.is_empty() {
            encoded.push('\x1f');
        }
        encoded.push_str(&flag);
    }
    command.env("CARGO_ENCODED_RUSTFLAGS", encoded);
}

/// 호스트를 릴리스로 빌드해 crates/app/binaries/linkgroupz-host-<triple>[.exe] 로 복사한다.
/// Tauri 가 번들을 만들 때 이 파일을 앱 실행 파일 옆(macOS 는 Contents/MacOS)에 넣는다.
pub fn build_host(target: Option<&str>) -> Outcome<PathBuf> {
    let root = workspace_root();
    let triple = match target {
        Some(target) => target.to_owned(),
        None => host_triple()?,
    };
    let extension = if triple.contains("windows") { ".exe" } else { "" };
    let binaries = root.join("crates/app/binaries");
    fs::create_dir_all(&binaries)?;
    let destination = binaries.join(format!("{HOST_NAME}-{triple}{extension}"));

    if triple == "universal-apple-darwin" {
        // Apple Silicon + Intel 을 하나로 합친다.
        let mut slices = Vec::new();
        for arch in ["aarch64-apple-darwin", "x86_64-apple-darwin"] {
            slices.push(build_host(Some(arch))?);
        }
        execute(Command::new("lipo").arg("-create").args(&slices).arg("-output").arg(&destination))?;
    } else {
        let mut build = Command::new(env!("CARGO"));
        build.current_dir(&root).args(["build", "--release", "-p", "linkgroupz-host"]);
        hide_local_paths(&mut build);
        let mut built = root.join("target");
        if target.is_some() {
            build.args(["--target", &triple]);
            built.push(&triple);
        }
        execute(&mut build)?;
        fs::copy(built.join("release").join(format!("{HOST_NAME}{extension}")), &destination)?;
    }
    println!("호스트: {}", destination.display());
    Ok(destination)
}

fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Tauri CLI. 설치돼 있으면 `cargo tauri`, 없으면 미리 빌드된 npm 패키지(`npx @tauri-apps/cli`)를 쓴다.
pub fn tauri_cli() -> Command {
    let has_cargo_tauri = Command::new(env!("CARGO"))
        .args(["tauri", "--version"])
        .output()
        .is_ok_and(|output| output.status.success());
    if has_cargo_tauri {
        let mut command = Command::new(env!("CARGO"));
        command.arg("tauri");
        command
    } else {
        let npx = if cfg!(windows) { "npx.cmd" } else { "npx" };
        let mut command = Command::new(npx);
        command.args(["--yes", "@tauri-apps/cli@2"]);
        command
    }
}

pub fn run(options: &[String]) -> Outcome {
    let root = workspace_root();
    let universal = has_option(options, "--universal");
    let target = if universal { Some("universal-apple-darwin".to_owned()) } else { option_value(options, "--target") };
    build_host(target.as_deref())?;

    let mut build = tauri_cli();
    build.current_dir(root.join("crates/app")).arg("build");
    hide_local_paths(&mut build);
    if let Some(target) = &target {
        build.args(["--target", target]);
    }
    // macOS 는 .app 까지만 Tauri 에게 맡기고, DMG 는 사용법 폴더를 함께 넣기 위해 아래에서 직접 만든다.
    build.args(["--bundles", if cfg!(target_os = "macos") { "app" } else { "nsis" }]);
    execute(&mut build)?;

    let mut bundle = root.join("target");
    if let Some(target) = &target {
        bundle.push(target);
    }
    bundle.push("release/bundle");
    let dist = root.join("dist");
    fs::create_dir_all(&dist)?;

    if cfg!(target_os = "macos") {
        let app = bundle.join("macos/LinkGroupz.app");
        let staging = dist.join("dmg-staging");
        let dmg = dist.join(format!("LinkGroupz-{}.dmg", version()));
        let _ = fs::remove_dir_all(&staging);
        let _ = fs::remove_file(&dmg);
        fs::create_dir_all(&staging)?;
        // 서명 · 확장 속성 · 심볼릭 링크를 그대로 옮기려고 ditto 를 쓴다.
        execute(Command::new("ditto").arg(&app).arg(staging.join("LinkGroupz.app")))?;
        execute(Command::new("ln").args(["-s", "/Applications"]).arg(staging.join("Applications")))?;
        execute(Command::new("ditto").arg(root.join("docs/guide")).arg(staging.join("사용법 · Guide")))?;
        execute(Command::new("hdiutil")
            .args(["create", "-volname", &format!("LinkGroupz {}", version()), "-srcfolder"])
            .arg(&staging)
            .args(["-ov", "-format", "UDZO"])
            .arg(&dmg))?;
        fs::remove_dir_all(&staging)?;
        println!("설치 파일: {}", dmg.display());

        if has_option(options, "--install") {
            let installed = PathBuf::from("/Applications/LinkGroupz.app");
            let _ = Command::new("osascript").args(["-e", "tell application id \"dev.linkgroupz.LinkGroupz\" to quit"]).status();
            let _ = fs::remove_dir_all(&installed);
            execute(Command::new("ditto").arg(&app).arg(&installed))?;
            println!("설치: {}", installed.display());
        }
    } else {
        // Tauri 가 만든 설치 파일(LinkGroupz_<버전>_x64-setup.exe)을 앱이 버전을 읽을 수 있는 이름으로 복사한다.
        let nsis = bundle.join("nsis");
        let setup = fs::read_dir(&nsis)?
            .filter_map(|entry| Some(entry.ok()?.path()))
            .find(|path| path.file_name().is_some_and(|name| name.to_string_lossy().ends_with("-setup.exe")))
            .ok_or_else(|| format!("{} 에 설치 파일이 없습니다", nsis.display()))?;
        let installer = dist.join(format!("LinkGroupz-{}-setup.exe", version()));
        fs::copy(setup, &installer)?;
        println!("설치 파일: {}", installer.display());
    }
    Ok(())
}
