use std::collections::HashMap;
use std::path::Path;

fn main() {
    bake_update_source();
    tauri_build::build();
}

/// 배포 파일이 올라오는 GitHub 저장소와 서명 공개 키를 빌드에 넣는다.
///
/// 이 값들은 소스에 두지 않는다. 빌드할 때 환경 변수로 주거나, 작업 공간 맨 위의 `update-source.env`
/// (git 에 올리지 않는 파일, `update-source.env.example` 참고)에 적어 둔다. 둘 다 없으면 자동 업데이트 없이 빌드된다.
/// (값이 올바른지는 앱이 켜질 때 `linkgroupz_core::UpdateSource::new` 가 본다.)
fn bake_update_source() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = root.join("update-source.env");
    // 파일이 없을 때 없는 파일을 지켜보라고 하면 매번 다시 빌드된다. 그때는 폴더를 지켜봐 파일이 생기면 알게 한다.
    let watched = if file.exists() { file.clone() } else { root.clone() };
    println!("cargo:rerun-if-changed={}", watched.display());

    let from_file: HashMap<String, String> = std::fs::read_to_string(&file)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_owned(), value.trim().trim_matches('"').to_owned()))
        .collect();

    for (name, built_in) in [
        ("LINKGROUPZ_RELEASES_REPO", "LINKGROUPZ_BUILT_IN_RELEASES_REPO"),
        ("LINKGROUPZ_UPDATE_PUBKEY", "LINKGROUPZ_BUILT_IN_UPDATE_PUBKEY"),
    ] {
        println!("cargo:rerun-if-env-changed={name}");
        let value = std::env::var(name).ok().or_else(|| from_file.get(name).cloned()).filter(|value| !value.trim().is_empty());
        if let Some(value) = value {
            println!("cargo:rustc-env={built_in}={}", value.trim());
        }
    }
}
