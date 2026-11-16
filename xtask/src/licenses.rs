//! 앱에 든 외부 코드의 저작권 · 라이선스 고지(THIRD-PARTY-LICENSES.html)를 만든다.
//!
//! MIT · Apache-2.0 · BSD 같은 라이선스는 배포물에 저작권 문구와 라이선스 전문을 함께 넣으라고 요구한다.
//! 크레이트 목록과 라이선스 글은 cargo-about 이 모으고(설정: about.toml), 여기서는 cargo-about 이
//! 라이선스 파일을 알아보지 못한 크레이트를 한 번 더 찾아본 뒤 화면 쪽 외부 코드와 합쳐 한 파일로 쓴다.

use crate::{has_option, workspace_root, Outcome};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;

/// 만들어지는 파일 (작업 공간 맨 위). Tauri 가 앱 리소스로 넣는다 (crates/app/tauri.conf.json).
pub const OUTPUT: &str = "THIRD-PARTY-LICENSES.html";

/// 화면(`crates/app/ui`)에 파일째 넣은 외부 코드. crates/app/ui/vendor/README.md 와 같아야 한다.
const UI_COMPONENTS: [UiComponent; 3] = [
    UiComponent { name: "Preact", version: "10", license: "MIT", url: "https://preactjs.com", file: "LICENSE-preact.txt" },
    UiComponent { name: "htm", version: "3.1.1", license: "Apache-2.0", url: "https://github.com/developit/htm", file: "LICENSE-htm.txt" },
    UiComponent { name: "Phosphor Icons", version: "2.1.1", license: "MIT", url: "https://phosphoricons.com", file: "LICENSE-phosphor-icons.txt" },
];

struct UiComponent {
    name: &'static str,
    version: &'static str,
    license: &'static str,
    url: &'static str,
    file: &'static str,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Krate {
    name: String,
    version: String,
    repository: Option<String>,
}

/// 같은 라이선스 글을 쓰는 크레이트 묶음
struct Notice {
    id: String,
    name: String,
    text: String,
    /// 패키지에 라이선스 파일이 없어 표준 문구를 실었다
    standard_text: bool,
    crates: Vec<Krate>,
}

/// `cargo xtask licenses [--check]`
pub fn run(options: &[String]) -> Outcome {
    let root = workspace_root();
    let html = render(&collect(&root)?, &root)?;
    let file = root.join(OUTPUT);
    if has_option(options, "--check") {
        let current = fs::read_to_string(&file).unwrap_or_default();
        if current != html {
            return Err(format!("{OUTPUT} 이 지금 의존성과 다릅니다. `cargo xtask licenses` 로 다시 만드세요.").into());
        }
        println!("{OUTPUT}: 지금 의존성과 같습니다");
    } else {
        fs::write(&file, html)?;
        println!("라이선스 고지: {}", file.display());
    }
    Ok(())
}

/// cargo-about 으로 앱에 들어가는 크레이트와 라이선스 글을 모은다.
/// (호스트의 의존성은 앱의 의존성에 모두 들어 있어 앱만 본다.)
fn collect(root: &Path) -> Outcome<Vec<Notice>> {
    let json = root.join("target").join("third-party-licenses.json");
    fs::create_dir_all(root.join("target"))?;
    let status = Command::new(env!("CARGO"))
        .current_dir(root)
        .args(["about", "generate", "--config", "about.toml", "--manifest-path", "crates/app/Cargo.toml"])
        .args(["--locked", "--fail", "--format", "json", "--output-file"])
        .arg(&json)
        .status()
        .map_err(|error| format!("cargo about 을 실행하지 못했습니다: {error}"))?;
    if !status.success() {
        return Err("cargo about 이 실패했습니다. 설치: `cargo install --locked cargo-about --features cli`. \
                    받아들이지 않는 라이선스가 들어왔다면 about.toml 의 accepted 를 확인하세요."
            .into());
    }
    let report: Value = serde_json::from_str(&fs::read_to_string(&json)?)?;

    // (라이선스 ID, 라이선스 글) 이 같으면 한 묶음으로 합친다.
    let mut notices: BTreeMap<(String, String), Notice> = BTreeMap::new();
    let mut add = |id: &str, name: &str, text: String, standard_text: bool, krate: Krate| {
        let notice = notices.entry((id.to_owned(), text.clone())).or_insert_with(|| Notice {
            id: id.to_owned(),
            name: name.to_owned(),
            text,
            standard_text,
            crates: Vec::new(),
        });
        notice.crates.push(krate);
    };

    for license in report["licenses"].as_array().ok_or("cargo about 결과에 licenses 가 없습니다")? {
        let id = text_of(&license["id"]);
        let name = text_of(&license["name"]);
        let found_in_package = license["source_path"].is_string();
        for user in license["used_by"].as_array().into_iter().flatten() {
            let package = &user["crate"];
            let krate = Krate {
                name: text_of(&package["name"]),
                version: text_of(&package["version"]),
                repository: package["repository"].as_str().map(str::to_owned),
            };
            if found_in_package {
                add(&id, &name, normalize(&text_of(&license["text"])), false, krate);
                continue;
            }
            // cargo-about 이 라이선스 파일을 알아보지 못해 표준 문구를 준 크레이트.
            // 파일 이름이 흔한 모양이 아닐 뿐 패키지 안에 있는 경우가 많다 (license-mit, LICENSE_MIT …).
            let directory = Path::new(package["manifest_path"].as_str().unwrap_or_default()).parent().map(Path::to_owned);
            match directory.as_deref().and_then(|directory| packaged_license(directory, &id)) {
                Some(text) => add(&id, &name, text, false, krate),
                None => {
                    let text = standard_text(&text_of(&license["text"]), &copyright_holders(package, &krate));
                    add(&id, &name, text, true, krate);
                }
            }
        }
    }

    let mut notices: Vec<Notice> = notices.into_values().collect();
    for notice in &mut notices {
        notice.crates.sort();
        notice.crates.dedup();
    }
    notices.sort_by(|a, b| (&a.id, &a.crates[0], &a.text).cmp(&(&b.id, &b.crates[0], &b.text)));
    Ok(notices)
}

fn text_of(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

/// 줄 끝을 LF 로 맞추고 줄 끝 공백 · 앞뒤 빈 줄을 뗀다 (같은 글이 한 묶음이 되게).
fn normalize(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    let first = lines.iter().position(|line| !line.is_empty()).unwrap_or(0);
    let last = lines.iter().rposition(|line| !line.is_empty()).map_or(0, |index| index + 1);
    lines[first..last.max(first)].join("\n")
}

/// 라이선스 ID 를 알아볼 수 있는 파일 이름 조각과, 그 라이선스의 글이면 반드시 들어 있는 문구
fn license_marks(id: &str) -> Option<(&'static [&'static str], &'static str)> {
    Some(match id {
        "MIT" => (&["mit"], "Permission is hereby granted, free of charge"),
        "Apache-2.0" => (&["apache"], "Apache License"),
        "BSD-3-Clause" | "BSD-2-Clause" => (&["bsd"], "Redistribution and use in source and binary forms"),
        "BSL-1.0" => (&["boost", "bsl"], "Boost Software License"),
        "Zlib" => (&["zlib"], "This software is provided 'as-is'"),
        "MPL-2.0" => (&["mpl"], "Mozilla Public License"),
        "ISC" => (&["isc"], "Permission to use, copy, modify, and/or distribute"),
        _ => return None,
    })
}

/// 패키지 폴더에서 이 라이선스의 파일을 찾아 그 글을 돌려준다.
fn packaged_license(directory: &Path, id: &str) -> Option<String> {
    let (name_marks, phrase) = license_marks(id)?;
    let mut files: Vec<(String, std::path::PathBuf)> = fs::read_dir(directory)
        .ok()?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_name()?.to_string_lossy().to_lowercase();
            let is_license = ["license", "licence", "copying"].iter().any(|prefix| name.starts_with(prefix));
            (path.is_file() && is_license && !name.ends_with(".spdx")).then_some((name, path))
        })
        .collect();
    files.sort();
    // 이름에 라이선스가 드러난 파일(LICENSE-MIT)을 먼저, 없으면 이름만으로는 알 수 없는 파일(LICENSE)을 본다.
    let named = files.iter().filter(|(name, _)| name_marks.iter().any(|mark| name.contains(mark)));
    let unnamed = files.iter().filter(|(name, _)| {
        let stem = name.split('.').next().unwrap_or_default();
        matches!(stem, "license" | "licence" | "copying")
    });
    named
        .chain(unnamed)
        .filter_map(|(_, path)| fs::read_to_string(path).ok())
        .find(|text| text.contains(phrase))
        .map(|text| normalize(&text))
}

/// 저작권자로 적을 이름: Cargo.toml 의 authors, 없으면 저장소 이름.
fn copyright_holders(package: &Value, krate: &Krate) -> String {
    let authors: Vec<String> = package["authors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        // "이름 <메일>" 에서 메일은 뺀다.
        .map(|author| author.split('<').next().unwrap_or_default().trim().to_owned())
        .filter(|author| !author.is_empty())
        .collect();
    if !authors.is_empty() {
        return authors.join(", ");
    }
    let project = krate
        .repository
        .as_deref()
        .and_then(|repository| repository.trim_end_matches('/').rsplit('/').next())
        .map(|name| name.trim_end_matches(".git"))
        .filter(|name| !name.is_empty())
        .unwrap_or(&krate.name);
    format!("the {project} contributors")
}

/// 표준 문구의 저작권자 빈칸(`<year> <copyright holders>` 등)을 빼고 맨 위에 저작권자를 적는다.
fn standard_text(text: &str, holders: &str) -> String {
    let is_placeholder = |line: &str| {
        let line = line.trim();
        line.starts_with("Copyright") && (line.contains('<') || line.contains('['))
    };
    let mut body: Vec<&str> = text.lines().map(str::trim_end).filter(|line| !is_placeholder(line)).collect();
    // 빈칸 줄을 뺀 자리에 빈 줄이 겹치지 않게 한다.
    body.dedup_by(|line, previous| line.is_empty() && previous.is_empty());
    normalize(&format!("Copyright (c) {holders}\n\n{}", normalize(&body.join("\n"))))
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

const STYLE: &str = r#"
:root { color-scheme: light dark; --text: #1d1d1f; --secondary: #6e6e73; --line: #d2d2d7; --panel: #f5f5f7; --link: #0066cc; --page: #ffffff; }
@media (prefers-color-scheme: dark) {
  :root { --text: #f5f5f7; --secondary: #a1a1a6; --line: #424245; --panel: #1d1d1f; --link: #4da3ff; --page: #000000; }
}
* { box-sizing: border-box; }
body { margin: 0; padding: 32px 16px 64px; background: var(--page); color: var(--text);
  font: 15px/1.6 -apple-system, BlinkMacSystemFont, "Segoe UI", "Malgun Gothic", sans-serif; }
main { max-width: 860px; margin: 0 auto; }
h1 { font-size: 26px; margin: 0 0 8px; }
h2 { font-size: 19px; margin: 40px 0 12px; padding-top: 24px; border-top: 1px solid var(--line); }
h3 { font-size: 15px; margin: 28px 0 6px; }
h3 small { color: var(--secondary); font-weight: 400; }
p { margin: 8px 0; }
a { color: var(--link); text-decoration: none; }
a:hover { text-decoration: underline; }
.secondary { color: var(--secondary); font-size: 13px; }
table { border-collapse: collapse; }
td, th { text-align: left; padding: 4px 24px 4px 0; border-bottom: 1px solid var(--line); }
td.count { text-align: right; font-variant-numeric: tabular-nums; }
ul.crates { list-style: none; margin: 0 0 8px; padding: 0; display: flex; flex-wrap: wrap; gap: 2px 14px; font-size: 13px; }
ul.crates span { color: var(--secondary); }
pre { margin: 0; padding: 12px 14px; background: var(--panel); border: 1px solid var(--line); border-radius: 8px;
  font: 12px/1.5 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; white-space: pre-wrap; overflow-wrap: anywhere; }
"#;

fn render(notices: &[Notice], root: &Path) -> Outcome<String> {
    let mut html = String::new();
    let crate_count = {
        let mut all: Vec<&Krate> = notices.iter().flat_map(|notice| &notice.crates).collect();
        all.sort();
        all.dedup();
        all.len()
    };

    write!(
        html,
        "<!doctype html>\n<html lang=\"ko\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>LinkGroupz 오픈소스 라이선스</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<main>\n\
         <h1>LinkGroupz 오픈소스 라이선스</h1>\n\
         <p>LinkGroupz 에는 아래 외부 코드가 들어 있습니다. 각 코드의 저작권 문구와 라이선스 전문을 싣습니다.</p>\n\
         <p class=\"secondary\">Rust 크레이트 {crate_count}개 (macOS · Windows 용 앱에 들어가는 것과 빌드에 쓰이는 것), \
         화면에 쓰인 코드 · 그림 {}개. 여러 라이선스 가운데 고를 수 있는 크레이트는 그중 하나를 골라 실었습니다. \
         이 파일은 <code>cargo xtask licenses</code> 가 만듭니다.</p>\n",
        UI_COMPONENTS.len()
    )?;

    // 요약: 라이선스마다 몇 개인지
    let mut counts: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for notice in notices {
        *counts.entry((&notice.id, &notice.name)).or_default() += notice.crates.len();
    }
    for component in &UI_COMPONENTS {
        let name = notices.iter().find(|notice| notice.id == component.license).map_or(component.license, |notice| &notice.name);
        *counts.entry((component.license, name)).or_default() += 1;
    }
    html.push_str("<h2>요약</h2>\n<table>\n<tr><th>라이선스</th><th>개수</th></tr>\n");
    let mut rows: Vec<_> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for ((id, name), count) in rows {
        writeln!(html, "<tr><td>{} <span class=\"secondary\">({})</span></td><td class=\"count\">{count}</td></tr>", escape(name), escape(id))?;
    }
    html.push_str("</table>\n");

    html.push_str("<h2>화면에 쓰인 코드 · 그림</h2>\n");
    for component in &UI_COMPONENTS {
        let file = root.join("crates/app/ui/vendor").join(component.file);
        let text = fs::read_to_string(&file).map_err(|error| format!("{} 을 읽지 못했습니다: {error}", file.display()))?;
        write!(
            html,
            "<h3><a href=\"{}\">{}</a> {} <small>({})</small></h3>\n<pre>{}</pre>\n",
            escape(component.url),
            escape(component.name),
            escape(component.version),
            escape(component.license),
            escape(&normalize(&text))
        )?;
    }

    html.push_str("<h2>Rust 크레이트</h2>\n");
    for notice in notices {
        writeln!(html, "<h3>{} <small>({})</small></h3>", escape(&notice.name), escape(&notice.id))?;
        html.push_str("<ul class=\"crates\">\n");
        for krate in &notice.crates {
            let link = krate
                .repository
                .clone()
                .filter(|repository| repository.starts_with("https://"))
                .unwrap_or_else(|| format!("https://crates.io/crates/{}", krate.name));
            writeln!(html, "<li><a href=\"{}\">{}</a> <span>{}</span></li>", escape(&link), escape(&krate.name), escape(&krate.version))?;
        }
        html.push_str("</ul>\n");
        if notice.id == "MPL-2.0" {
            html.push_str("<p class=\"secondary\">LinkGroupz 는 이 코드를 고치지 않고 씁니다. 소스는 위 링크의 저장소에서 받을 수 있습니다.</p>\n");
        }
        if notice.standard_text {
            html.push_str("<p class=\"secondary\">패키지에 라이선스 파일이 들어 있지 않아, 패키지에 적힌 저작자와 라이선스의 표준 문구를 실었습니다.</p>\n");
        }
        writeln!(html, "<pre>{}</pre>", escape(&notice.text))?;
    }
    html.push_str("</main>\n</body>\n</html>\n");
    Ok(html)
}
