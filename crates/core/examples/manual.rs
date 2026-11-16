//! 실제 브라우저 · 운영체제를 건드리는 수동 확인.
//!
//! ```text
//! cargo run -p linkgroupz-core --example manual -- <옵션>
//!   --print-marker          확장(JS) 디코더 교차 확인용 마커 URL 출력
//!   --browsers              설치 · 실행 여부, 켜진 시각, 아이콘 크기
//!   --detect                브라우저 프로필에서 확장 감지
//!   --session-file          브라우저 세션 파일에서 읽은 창 · 탭 · 탭 그룹 수 (내용은 출력하지 않는다)
//!   --script-selftest       AppleScript 결과 변환 확인 (macOS, 다른 앱을 건드리지 않는다)
//!   --read-tabs [--edge]    열린 탭 수 읽기 (macOS, 자동화 권한 필요)
//!   --open-demo             데모 그룹(새 창)과 단일 페이지를 연다
//!   --open-demo-tabgroup    확장을 거쳐 데모 그룹을 탭 그룹으로 연다
//! ```

use linkgroupz_core::*;
use std::collections::HashSet;
use std::thread::sleep;
use std::time::Duration;

fn link(url: &str) -> SavedLink {
    SavedLink::new(LinkUrl::new(url), None)
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| arguments.iter().any(|argument| argument == flag);
    if arguments.is_empty() || has("--help") {
        println!("옵션: --print-marker --browsers --detect --script-selftest --read-tabs [--edge] --open-demo --open-demo-tabgroup");
        return;
    }

    if has("--print-marker") {
        let group = LinkGroup::new("업무 / 테스트").with_color(GroupColor::Purple).with_links(vec![
            SavedLink::new(SavedLink::parse_url("https://example.com/a?b=1&c=한글").expect("주소"), None),
            link("https://example.org"),
        ]);
        println!("{}", TabGroupMarker::url(&group));
    }

    if has("--browsers") {
        for browser in Browser::ALL {
            println!(
                "{}: installed={} path={:?} running={} launched={:?} icon={:?}",
                browser.raw_value(),
                browser.is_installed(),
                browser.app_path(),
                browser.is_running(),
                browser.launch_date().map(Timestamp::unix_seconds),
                browser.icon_png(32).map(|png| {
                    let image = image::load_from_memory(&png).expect("PNG");
                    format!("{}x{} ({} bytes)", image.width(), image.height(), png.len())
                }),
            );
        }
    }

    if has("--session-file") {
        for browser in Browser::ALL {
            match linkgroupz_core::session_file::SessionFileReader::read(browser) {
                Some(snapshot) => println!(
                    "session {}: 창 {} · 탭 {} · 탭 그룹 {} (그룹별 탭 {:?})",
                    browser.raw_value(),
                    snapshot.windows.len(),
                    snapshot.windows.iter().map(|window| window.tabs.len()).sum::<usize>(),
                    snapshot.groups.len(),
                    snapshot.tab_groups().iter().map(|group| group.tabs.len()).collect::<Vec<_>>()
                ),
                None => println!("session {}: 없음", browser.raw_value()),
            }
        }
    }

    if has("--detect") {
        for browser in Browser::ALL {
            println!("detect {}: {:?}", browser.raw_value(), ChromeExtensionDetector::find(browser));
        }
    }

    if has("--script-selftest") {
        let host = DirectTabsHost;
        let value = host.run_script(r#"return {{1, "제목", "https://x.com"}, {2, "", "https://y.com"}, {true, 3}}"#);
        println!("list: {value:?}");
        let tabs = value.as_ref().map(ChromeTabsReader::parse).unwrap_or_default();
        println!("tabs: {tabs:?}");
        println!("text: {:?}", host.run_script(r#"return "https://linkgroupz.invalid/result#abc""#));
        println!("error: {:?}", host.run_script(r#"error "no window" number 9001"#).map_err(TabsError::from));
        println!("syntax: {:?}", host.run_script("this is not applescript ((").map_err(TabsError::from));
    }

    if has("--read-tabs") {
        let browser = if has("--edge") { Browser::Edge } else { Browser::Chrome };
        match ChromeTabsReader::read(browser, &DirectTabsHost) {
            Ok(tabs) => {
                let windows: HashSet<usize> = tabs.iter().map(|tab| tab.window_index).collect();
                println!("read-tabs: {} tabs in {} windows", tabs.len(), windows.len());
            }
            Err(error) => println!("read-tabs failed: {error}"),
        }
    }

    if has("--open-demo-tabgroup") {
        let group = LinkGroup::new("데모 그룹").with_color(GroupColor::Purple).with_links(vec![
            link("https://example.com/?linkgroupz-demo=tg1"),
            link("https://example.org/?linkgroupz-demo=tg2"),
        ]);
        if let Err(error) = ChromeLauncher::open_group(&group, true) {
            println!("open failed: {error}");
            std::process::exit(1);
        }
        sleep(Duration::from_secs(2));
    }

    if has("--open-demo") {
        let group = LinkGroup::new("데모").with_links(vec![
            link("https://example.com/?linkgroupz-demo=1"),
            link("https://example.org/?linkgroupz-demo=2"),
            link("https://example.net/?linkgroupz-demo=3"),
        ]);
        let opened = ChromeLauncher::open_group(&group, false).and_then(|()| {
            sleep(Duration::from_secs(2));
            ChromeLauncher::open_link(&link("https://example.com/?linkgroupz-demo=single"), Browser::DEFAULT)
        });
        if let Err(error) = opened {
            println!("open failed: {error}");
            std::process::exit(1);
        }
        sleep(Duration::from_secs(2));
    }
}
