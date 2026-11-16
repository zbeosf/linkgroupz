//! 빌드 보조 도구. `cargo xtask <명령>` 으로 부른다 (.cargo/config.toml 의 alias).
//!
//! ```text
//! cargo xtask icons            assets/*.svg 로 앱 · 메뉴바 아이콘을 만든다 (crates/app/icons/)
//! cargo xtask host [--target <triple>]
//!                              Native Messaging 호스트를 빌드해 앱에 함께 넣을 자리(crates/app/binaries/)에 둔다
//! cargo xtask dist [--universal] [--install]
//!                              배포 파일을 만든다 (macOS: dist/LinkGroupz-<버전>.dmg, Windows: dist/LinkGroupz-<버전>-setup.exe)
//! cargo xtask release [--windows <setup.exe>] [--mac-only] [--publish]
//!                              릴리스 묶음(서명 · latest.json 포함)을 dist/release-<버전>/ 에 만든다. --publish 는 GitHub 릴리스로 올린다
//! cargo xtask licenses [--check]
//!                              외부 코드의 저작권 · 라이선스 고지(THIRD-PARTY-LICENSES.html)를 다시 만든다.
//!                              --check 는 만들지 않고, 있는 파일이 지금 의존성과 같은지만 본다 (cargo-about 필요)
//! ```

mod dist;
mod icons;
mod licenses;
mod release;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (command, options) = match arguments.split_first() {
        Some((command, options)) => (command.as_str(), options),
        None => ("help", &[][..]),
    };
    let result = match command {
        "icons" => icons::generate(),
        "host" => dist::build_host(option_value(options, "--target").as_deref()).map(|_| ()),
        "dist" => dist::run(options),
        "licenses" => licenses::run(options),
        "release" => release::run(options),
        _ => {
            println!("사용: cargo xtask <icons | host [--target <triple>] | dist [--universal] [--install] | licenses [--check] | release [--windows <setup.exe>] [--mac-only] [--publish]>");
            return if command == "help" || command == "--help" { ExitCode::SUCCESS } else { ExitCode::FAILURE };
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("실패: {error}");
            ExitCode::FAILURE
        }
    }
}

pub type Outcome<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// 작업 공간 맨 위 폴더
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask 는 작업 공간 안에 있다").to_owned()
}

pub fn option_value(options: &[String], name: &str) -> Option<String> {
    options.iter().position(|option| option == name).and_then(|index| options.get(index + 1)).cloned()
}

pub fn has_option(options: &[String], name: &str) -> bool {
    options.iter().any(|option| option == name)
}

/// 명령을 실행하고, 실패하면 무엇이 실패했는지 알린다.
pub fn execute(command: &mut Command) -> Outcome {
    let description = format!("{command:?}");
    let status = command.status().map_err(|error| format!("{description} 을 실행하지 못했습니다: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{description} 이 실패했습니다 ({status})").into())
    }
}
