//! Swift 판의 CoreChecks 를 옮긴 검사. 실행: `cargo test -p linkgroupz-core`
//!
//! 실제 브라우저를 건드리는 수동 확인은 `cargo run -p linkgroupz-core --example manual -- --help` 에 있다.

use linkgroupz_core::*;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

fn link(url: &str) -> SavedLink {
    SavedLink::new(LinkUrl::new(url), None)
}

fn link_alias(url: &str, alias: &str) -> SavedLink {
    SavedLink::new(LinkUrl::new(url), Some(alias))
}

fn ids<const N: usize>(ids: [Id; N]) -> HashSet<Id> {
    HashSet::from(ids)
}

/// 검사마다 따로 쓰는 임시 폴더 (끝나면 지워진다)
struct Scratch(tempfile::TempDir);

impl Scratch {
    fn new() -> Self {
        Self(tempfile::Builder::new().prefix("LinkGroupzChecks-").tempdir().expect("임시 폴더"))
    }

    fn path(&self) -> &Path {
        self.0.path()
    }

    fn data_file(&self) -> PathBuf {
        self.path().join("store").join("data.json")
    }
}

fn sample_groups() -> Vec<LinkGroup> {
    vec![
        LinkGroup::new("업무").with_color(GroupColor::Blue).with_tags(&["work", "daily"]).with_links(vec![
            link_alias("https://jira.example.com/board", "지라 보드"),
            link("https://wiki.example.com/home"),
        ]),
        LinkGroup::new("뉴스").with_color(GroupColor::Red).with_tags(&["read"]).with_links(vec![
            link_alias("https://news.ycombinator.com", "HN"),
            link("https://www.reddit.com/r/swift"),
        ]),
    ]
}

fn names(groups: &[LinkGroup]) -> Vec<&str> {
    groups.iter().map(|group| group.name.as_str()).collect()
}

fn display_names(links: &[SavedLink]) -> Vec<String> {
    links.iter().map(SavedLink::display_name).collect()
}

fn aliases(links: &[SavedLink]) -> Vec<Option<&str>> {
    links.iter().map(|link| link.alias.as_deref()).collect()
}

const LEGACY_GROUP: &str =
    r#"{"id":"6F1C1E2E-6F0E-4C55-9C7A-0E5C1C0B9E11","name":"옛 그룹","color":"blue","tags":[],"links":[]}"#;

// MARK: - Models

#[test]
fn parse_url() {
    let parsed = |text: &str| SavedLink::parse_url(text).map(|url| url.as_str().to_owned());
    assert_eq!(parsed("example.com/a").as_deref(), Some("https://example.com/a"), "스킴 없으면 https");
    assert_eq!(parsed("  http://x.com ").as_deref(), Some("http://x.com"), "공백 제거");
    assert_eq!(parsed(""), None, "빈 문자열");
    assert_eq!(parsed("not a url"), None, "공백 포함");
    assert_eq!(parsed("javascript:alert(1)"), None, "허용 안 하는 스킴");
    // 경로 없이 적은 주소는 그대로 두고, 적은 경로 · 쿼리는 지킨다
    assert_eq!(parsed("https://a.com?x=1").as_deref(), Some("https://a.com?x=1"), "쿼리만 있는 주소");
    assert_eq!(parsed("https://a.com/").as_deref(), Some("https://a.com/"), "적은 루트 경로는 그대로");
    assert_eq!(parsed("a.com/한글?q=값").as_deref(), Some("https://a.com/%ED%95%9C%EA%B8%80?q=%EA%B0%92"), "한글은 % 인코딩");
    assert_eq!(parsed("file:///tmp/a.html").as_deref(), Some("file:///tmp/a.html"), "파일 주소");
    assert_eq!(parsed("ftp://a.com/x"), None, "http(s) · file 만");
    assert_eq!(parsed("https://"), None, "호스트 없음");
}

#[test]
fn display_name() {
    assert_eq!(link_alias("https://a.com/x", "  별칭 ").display_name(), "별칭", "alias 우선");
    assert_eq!(link_alias("https://a.com/x", "   ").display_name(), "a.com/x", "빈 alias 는 host+path");
    assert_eq!(link("https://a.com/").display_name(), "a.com", "루트 경로는 host");
    assert_eq!(link("https://a.com").display_name(), "a.com", "경로 없음");
    assert_eq!(link("https://a.com/%ED%95%9C%EA%B8%80/").display_name(), "a.com/한글", "경로는 풀어서, 끝의 / 없이");
}

#[test]
fn tags() {
    assert_eq!(LinkGroup::parse_tags("work, #Daily  work daily,,"), ["work", "Daily"], "태그 정규화");
}

#[test]
fn array_move() {
    let mut a = vec![0, 1, 2, 3, 4];
    move_elements(&mut a, &[1, 3], 5);
    assert_eq!(a, [0, 2, 4, 1, 3], "뒤로 이동");
    let mut b = vec![0, 1, 2, 3, 4];
    move_elements(&mut b, &[4], 0);
    assert_eq!(b, [4, 0, 1, 2, 3], "앞으로 이동");
    let mut c = vec![0, 1, 2];
    move_elements(&mut c, &[0], 2);
    assert_eq!(c, [1, 0, 2], "한 칸 아래");
    let mut d = vec![0, 1, 2];
    move_elements(&mut d, &[7], 1);
    assert_eq!(d, [0, 1, 2], "없는 자리는 무시");
}

#[test]
fn id_format() {
    let id = Id::parse("6f1c1e2e-6f0e-4c55-9c7a-0e5c1c0b9e11").expect("소문자도 읽는다");
    assert_eq!(id.to_string(), "6F1C1E2E-6F0E-4C55-9C7A-0E5C1C0B9E11", "Swift UUID 처럼 대문자로 적는다");
    assert_eq!(serde_json::to_string(&id).unwrap(), "\"6F1C1E2E-6F0E-4C55-9C7A-0E5C1C0B9E11\"");
    assert_eq!(serde_json::from_str::<Id>("\"6F1C1E2E-6F0E-4C55-9C7A-0E5C1C0B9E11\"").unwrap(), id);
    assert!(Id::parse("not-a-uuid").is_none());
}

// MARK: - LinkStore

#[test]
fn store_persists() {
    let scratch = Scratch::new();
    let mut store = LinkStore::open(scratch.data_file());
    let group = store.add_group(LinkGroup::new("업무").with_color(GroupColor::Green).with_tags(&["work"]));
    store.add_link(link_alias("https://a.com", "A"), group.id);
    store.add_link(link("https://b.com"), group.id);

    let restored = LinkStore::open(scratch.data_file());
    assert_eq!(restored.groups().len(), 1, "그룹 복원");
    assert_eq!(restored.groups()[0].color, GroupColor::Green, "색 복원");
    assert_eq!(restored.groups()[0].tags, ["work"], "태그 복원");
    assert_eq!(display_names(&restored.groups()[0].links), ["A", "b.com"], "링크 복원");
    assert!(store.last_error().is_none());
}

#[test]
fn archive() {
    let scratch = Scratch::new();
    let mut store = LinkStore::open(scratch.data_file());
    let work = store.add_group(LinkGroup::new("업무").with_color(GroupColor::Green).with_links(vec![
        link_alias("https://a.com", "A"),
        link_alias("https://b.com", "B"),
        link_alias("https://c.com", "C"),
    ]));
    let news = store.add_group(LinkGroup::new("뉴스").with_color(GroupColor::Red).with_links(vec![link("https://n.com")]));
    let other = store.add_group(LinkGroup::new("기타").with_links(vec![link("https://b.com/")]));

    // 페이지 보관: 그룹에서 빠지고 보관함 맨 위에, 원래 그룹 정보와 함께
    let (a_id, b_id) = (work.links[0].id, work.links[1].id);
    store.archive_links(&ids([a_id]), work.id, Timestamp::from_unix_seconds(1.0));
    store.archive_links(&ids([b_id]), work.id, Timestamp::from_unix_seconds(2.0));
    assert_eq!(display_names(&store.group(work.id).unwrap().links), ["C"], "보관한 페이지는 그룹에서 빠짐");
    let archived: Vec<String> = store.archive().links.iter().map(|item| item.link.display_name()).collect();
    assert_eq!(archived, ["B", "A"], "최근 보관이 위");
    let first = &store.archive().links[0];
    assert!(first.group_name == "업무" && first.group_color == GroupColor::Green, "원래 그룹 이름 · 색");

    // 그룹 보관: 목록 · 검색에서 빠짐
    store.archive_groups(&ids([news.id]), Timestamp::now());
    assert_eq!(names(store.groups()), ["업무", "기타"], "보관한 그룹은 목록에서 빠짐");
    let archived_groups: Vec<&str> = store.archive().groups.iter().map(|item| item.group.name.as_str()).collect();
    assert_eq!(archived_groups, ["뉴스"], "보관함에 그룹");
    assert!(SearchFilter::new("뉴스").apply(store.groups()).is_empty(), "보관한 그룹은 검색 안 됨");
    assert_eq!(store.groups_including_archive().len(), 3, "아이콘 정리용 전체 그룹");

    // 저장 · 다시 읽기
    let reloaded = LinkStore::open(scratch.data_file());
    assert_eq!(reloaded.archive(), store.archive(), "보관함 저장 · 복원");

    // 페이지 복원: 같은 주소가 이미 있으면 보관함에서만 뺌
    let result = store.restore_links(&ids([a_id, b_id]), other.id);
    assert_eq!(result, MoveResult { moved: 1, duplicates: 1 }, "복원 결과");
    assert_eq!(display_names(&store.group(other.id).unwrap().links), ["b.com", "A"], "중복은 넣지 않음");
    assert!(store.archive().links.is_empty(), "복원한 페이지는 보관함에서 빠짐");

    // 새 그룹으로 페이지 복원
    let remaining: HashSet<Id> = store.group(work.id).unwrap().links.iter().map(|link| link.id).collect();
    store.archive_links(&remaining, work.id, Timestamp::now());
    let c_id = store.archive().links[0].id();
    let made = LinkGroup::new("업무").with_color(GroupColor::Green);
    let made_id = made.id;
    store.restore_links_to_new_group(&ids([c_id]), made);
    assert_eq!(display_names(&store.group(made_id).unwrap().links), ["C"], "새 그룹으로 복원");

    // 그룹 복원: 목록 끝으로
    store.restore_groups(&ids([news.id]));
    let last = store.groups().last().unwrap();
    assert!(last.name == "뉴스" && last.links.len() == 1, "그룹 복원은 끝에, 페이지 그대로");
    assert!(store.archive().is_empty(), "보관함 비움");

    // 영구 삭제
    store.archive_groups(&ids([news.id]), Timestamp::now());
    let made_link = store.group(made_id).unwrap().links[0].id;
    store.archive_links(&ids([made_link]), made_id, Timestamp::now());
    store.delete_archived_groups(&ids([news.id]));
    store.delete_archived_links(&ids([c_id]));
    assert!(store.archive().is_empty() && store.group(news.id).is_none(), "영구 삭제");

    // 보관함 검색
    let search_archive = LinkArchive {
        groups: vec![ArchivedGroup::new(
            LinkGroup::new("지난 프로젝트").with_tags(&["2025"]).with_links(vec![
                link_alias("https://www.notion.so", "기획 문서"),
                link_alias("https://www.figma.com", "디자인 시안"),
            ]),
            Timestamp::now(),
        )],
        links: vec![
            ArchivedLink::new(link_alias("https://news.ycombinator.com", "Hacker News"), &LinkGroup::new("뉴스"), Timestamp::now()),
            ArchivedLink::new(link_alias("https://www.apple.com/kr/", "Apple"), &LinkGroup::new("쇼핑"), Timestamp::now()),
        ],
    };
    assert_eq!(search_archive.filtered(&SearchFilter::new("")), search_archive, "빈 검색어는 그대로");
    let by_page = search_archive.filtered(&SearchFilter::new("figma"));
    assert!(display_names(&by_page.groups[0].group.links) == ["디자인 시안"] && by_page.links.is_empty(), "보관 그룹 안 페이지로 찾기");
    let by_group = search_archive.filtered(&SearchFilter::new("프로젝트"));
    assert_eq!(by_group.groups[0].group.links.len(), 2, "그룹 이름이 맞으면 그룹 전체");
    assert_eq!(search_archive.filtered(&SearchFilter::new("#2025")).groups.len(), 1, "태그로 찾기");
    let by_origin = search_archive.filtered(&SearchFilter::new("쇼핑"));
    let origin_names: Vec<String> = by_origin.links.iter().map(|item| item.link.display_name()).collect();
    assert!(origin_names == ["Apple"] && by_origin.groups.is_empty(), "보관 페이지는 원래 그룹 이름으로도 찾기");
    assert_eq!(search_archive.filtered(&SearchFilter::new("hacker news")).links.len(), 1, "여러 검색어는 모두 맞아야");
    assert!(search_archive.filtered(&SearchFilter::new("없는말")).is_empty(), "결과 없음");

    // 예전 data.json(archive 없음)
    let document: LinkGroupsDocument = serde_json::from_str(r#"{"version":1,"groups":[]}"#).unwrap();
    assert!(document.archive.is_empty(), "archive 없는 파일은 빈 보관함");
}

#[test]
fn store_mutations() {
    let scratch = Scratch::new();
    let mut store = LinkStore::open(scratch.data_file());
    let a = store.add_group(LinkGroup::new("A"));
    let b = store.add_group(LinkGroup::new("B"));
    store.move_groups(&[1], 0);
    assert_eq!(names(store.groups()), ["B", "A"], "그룹 순서 변경");

    let mut renamed = a.clone();
    renamed.name = "A2".to_owned();
    renamed.tags = vec!["#x".to_owned(), "x".to_owned()];
    store.update_group(renamed);
    assert_eq!(store.group(a.id).unwrap().name, "A2", "그룹 이름 수정");
    assert_eq!(store.group(a.id).unwrap().tags, ["x"], "그룹 태그 정규화");

    let l1 = link("https://1.com");
    let l2 = link("https://2.com");
    store.add_link(l1.clone(), b.id);
    store.add_link(l2.clone(), b.id);
    store.move_links_within(b.id, &[1], 0);
    let order: Vec<Id> = store.group(b.id).unwrap().links.iter().map(|link| link.id).collect();
    assert_eq!(order, [l2.id, l1.id], "링크 순서 변경");

    let mut edited = l1.clone();
    edited.alias = Some("하나".to_owned());
    store.update_link(edited, b.id);
    assert_eq!(store.group(b.id).unwrap().links.last().unwrap().alias.as_deref(), Some("하나"), "링크 alias 수정");

    store.delete_link(l2.id, b.id);
    assert_eq!(store.group(b.id).unwrap().links.len(), 1, "링크 삭제");
    store.delete_group(a.id);
    assert_eq!(names(store.groups()), ["B"], "그룹 삭제");
}

#[test]
fn store_backs_up_unreadable_file() {
    let scratch = Scratch::new();
    let file = scratch.data_file();
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "not json").unwrap();
    let store = LinkStore::open(&file);
    assert!(store.groups().is_empty(), "빈 목록");
    assert!(store.last_error().is_some(), "오류 메시지");
    let files: Vec<String> =
        fs::read_dir(file.parent().unwrap()).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    assert!(files.iter().any(|name| name.starts_with("data.broken-")), "백업 파일 {files:?}");
}

#[test]
fn store_rejects_newer_file_version() {
    let scratch = Scratch::new();
    let file = scratch.data_file();
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, r#"{"version":2,"groups":[]}"#).unwrap();
    let store = LinkStore::open(&file);
    assert!(store.last_error().is_some_and(|error| error.contains("지원하지 않는 데이터 파일 버전")), "더 새 버전의 파일");
    assert!(!file.exists(), "덮어쓰지 않도록 백업으로 옮김");
}

#[test]
fn store_add_links() {
    let scratch = Scratch::new();
    let mut store = LinkStore::open(scratch.data_file());
    let group = store.add_group(LinkGroup::new("G"));
    store.add_links(vec![link("https://1.com"), link("https://2.com")], group.id);
    assert_eq!(store.group(group.id).unwrap().links.len(), 2, "여러 개 한 번에 추가");
}

#[test]
fn store_reloads_external_changes() {
    let scratch = Scratch::new();
    let file = scratch.data_file();
    let mut mine = LinkStore::open(&file);
    mine.add_group(LinkGroup::new("A"));
    assert!(!mine.reload_if_changed_on_disk(), "내가 쓴 내용은 다시 읽지 않음");

    // 다른 LinkGroupz(또는 직접 수정)가 파일을 바꿈
    let mut other = LinkStore::open(&file);
    other.add_group(LinkGroup::new("B"));
    assert!(mine.reload_if_changed_on_disk(), "바뀐 파일 다시 읽음");
    assert_eq!(names(mine.groups()), ["A", "B"], "다른 쪽에서 추가한 그룹이 보임");

    fs::write(&file, "쓰는 중").unwrap();
    assert!(!mine.reload_if_changed_on_disk(), "읽을 수 없는 내용이면 그대로");
    assert_eq!(mine.groups().len(), 2, "목록 유지");
}

#[test]
fn store_bulk_delete_and_move() {
    let scratch = Scratch::new();
    let file = scratch.data_file();
    let mut store = LinkStore::open(&file);
    let (l1, l2, l3, l4) = (link("https://1.com"), link("https://2.com"), link("https://3.com/"), link("https://4.com"));
    let a = store.add_group(LinkGroup::new("A").with_links(vec![l1.clone(), l2.clone(), l3.clone(), l4.clone()]));
    let b = store.add_group(LinkGroup::new("B").with_links(vec![link("https://3.com")]));
    let c = store.add_group(LinkGroup::new("C"));
    let d = store.add_group(LinkGroup::new("D"));

    let result = store.move_links(&ids([l3.id, l1.id]), a.id, b.id);
    assert_eq!(result, MoveResult { moved: 1, duplicates: 1 }, "같은 주소는 중복으로");
    let left: Vec<Id> = store.group(a.id).unwrap().links.iter().map(|link| link.id).collect();
    assert_eq!(left, [l2.id, l4.id], "원래 그룹에서 빠짐");
    let urls: Vec<&str> = store.group(b.id).unwrap().links.iter().map(|link| link.url.as_str()).collect();
    assert_eq!(urls, ["https://3.com", "https://1.com"], "대상 그룹 끝에 추가");
    assert_eq!(store.move_links(&ids([l2.id]), a.id, a.id), MoveResult::default(), "같은 그룹으로는 안 옮김");

    store.delete_links(&ids([l2.id, l4.id]), a.id);
    assert!(store.group(a.id).unwrap().links.is_empty(), "페이지 여러 개 삭제");
    store.delete_groups(&ids([c.id, d.id]));
    assert_eq!(names(store.groups()), ["A", "B"], "그룹 여러 개 삭제");
    assert_eq!(names(LinkStore::open(&file).groups()), ["A", "B"], "저장됨");
}

/// Swift 판이 쓴 data.json 을 그대로 읽고, 같은 모양으로 다시 쓴다.
#[test]
fn swift_data_file_compatibility() {
    let swift_written = r#"{
  "archive" : {
    "groups" : [
      {
        "archivedAt" : 784512345.25,
        "group" : {
          "alwaysPrivate" : false,
          "browser" : "edge",
          "color" : "navy",
          "icon" : { "image" : "A1B2.png" },
          "id" : "0D3F1B0A-2A0B-4C1D-8E2F-3A4B5C6D7E8F",
          "links" : [ ],
          "name" : "보관한 그룹",
          "tags" : [ ]
        }
      }
    ],
    "links" : [
      {
        "archivedAt" : 784512000.5,
        "groupColor" : "green",
        "groupID" : "6F1C1E2E-6F0E-4C55-9C7A-0E5C1C0B9E11",
        "groupName" : "업무",
        "link" : { "id" : "11111111-2222-4333-8444-555555555555", "url" : "https://b.com" }
      }
    ]
  },
  "groups" : [
    {
      "alwaysPrivate" : true,
      "browser" : "chrome",
      "color" : "blue",
      "icon" : { "symbol" : "server.rack" },
      "id" : "6F1C1E2E-6F0E-4C55-9C7A-0E5C1C0B9E11",
      "links" : [
        { "alias" : "지라", "id" : "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE", "url" : "https://jira.example.com/board" }
      ],
      "name" : "업무",
      "tags" : [ "work" ]
    }
  ],
  "version" : 1
}"#;
    let scratch = Scratch::new();
    let file = scratch.data_file();
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, swift_written).unwrap();

    let mut store = LinkStore::open(&file);
    assert!(store.last_error().is_none(), "읽기 오류 없음: {:?}", store.last_error());
    let group = &store.groups()[0];
    assert!(group.always_private && group.icon == GroupIcon::Symbol("server.rack".to_owned()), "그룹 속성");
    assert_eq!(group.links[0].alias.as_deref(), Some("지라"));
    let archived = &store.archive().groups[0];
    assert!(archived.group.browser == Browser::Edge && archived.group.icon == GroupIcon::Image("A1B2.png".to_owned()));
    // 2001-01-01 부터 지난 초 → 유닉스 초
    assert_eq!(archived.archived_at.unix_seconds(), 784512345.25 + 978_307_200.0, "Swift Date 형식의 시각");
    assert_eq!(store.archive().links[0].group_name, "업무");

    // 다시 쓴 파일: 같은 키 · 같은 값 (공백만 다르다)
    store.move_groups(&[0], 0);
    let rewritten: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    let original: serde_json::Value = serde_json::from_str(swift_written).unwrap();
    assert_eq!(rewritten, original, "Swift 판이 그대로 읽을 수 있는 모양");
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.find("\"archive\"").unwrap() < text.find("\"groups\"").unwrap(), "키는 이름순");
}

// MARK: - SearchFilter

#[test]
fn search() {
    let groups = sample_groups();
    assert_eq!(SearchFilter::new("   ").apply(&groups).len(), 2, "빈 검색어");

    let by_name = SearchFilter::new("업무").apply(&groups);
    assert!(names(&by_name) == ["업무"] && by_name[0].links.len() == 2, "그룹 이름 → 링크 전부");

    assert_eq!(names(&SearchFilter::new("#READ").apply(&groups)), ["뉴스"], "#태그");
    assert!(SearchFilter::new("#hn").apply(&groups).is_empty(), "#태그는 alias 를 보지 않음");

    let by_alias = SearchFilter::new("hn").apply(&groups);
    assert!(names(&by_alias) == ["뉴스"] && aliases(&by_alias[0].links) == [Some("HN")], "alias → 맞는 링크만");
    assert_eq!(SearchFilter::new("reddit").apply(&groups)[0].links.len(), 1, "URL → 맞는 링크만");

    assert_eq!(aliases(&SearchFilter::new("work 지라").apply(&groups)[0].links), [Some("지라 보드")], "그룹 태그 + alias → alias 맞는 링크만");
    assert_eq!(SearchFilter::new("#read swift").apply(&groups)[0].links.len(), 1, "#태그+URL");
    assert!(SearchFilter::new("hn reddit").apply(&groups).is_empty(), "서로 다른 링크에 나뉜 검색어");
}

#[test]
fn search_folding() {
    assert_eq!(SearchFilter::fold("ＡＢＣ　Ｄ"), "abc d", "전각 → 반각");
    assert_eq!(SearchFilter::fold("Café RÉSUMÉ"), "cafe resume", "발음 구별 기호 · 대소문자");
    assert_eq!(SearchFilter::fold("한글 각"), "한글 각", "한글은 그대로 (자모로 풀린 채 남지 않는다)");
    let groups = vec![LinkGroup::new("각도기"), LinkGroup::new("가나다")];
    assert_eq!(names(&SearchFilter::new("가").apply(&groups)), ["가나다"], "‘가’ 가 ‘각’ 에 맞지 않음");
    assert_eq!(names(&SearchFilter::new("＃").apply(&groups)).len(), 2, "# 만 있는 검색어는 무시");
}

// MARK: - TabGroupMarker

#[test]
fn marker_round_trip() {
    let group = &sample_groups()[0];
    let url = TabGroupMarker::url(group);
    let parsed = url::Url::parse(&url).unwrap();
    assert_eq!(parsed.host_str(), Some(TabGroupMarker::HOST), "마커 host");
    let fragment = parsed.fragment().unwrap_or("");
    assert!(!fragment.contains('+') && !fragment.contains('/') && !fragment.contains('='), "base64url");
    assert_eq!(
        TabGroupMarker::decode(&url),
        Some(MarkerPayload {
            name: "업무".to_owned(),
            color: "blue".to_owned(),
            urls: vec!["https://jira.example.com/board".to_owned(), "https://wiki.example.com/home".to_owned()],
        }),
        "디코드"
    );
    assert_eq!(TabGroupMarker::decode("https://example.com/open#e30"), None, "다른 호스트는 마커가 아님");
    assert_eq!(TabGroupMarker::decode(&url.replace("linkgroupz.invalid", "linkgroups.invalid")), None, "원본 확장의 마커 주소는 쓰지 않는다");
}

// MARK: - ChromeExtensionDetector

#[test]
fn extension_detector() {
    let scratch = Scratch::new();
    let root = scratch.path();

    let make_extension = |name: &str, manifest_name: &str| -> String {
        let dir = root.join(format!("ext-{name}"));
        fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({ "name": manifest_name, "manifest_version": 3 });
        fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
        dir.to_string_lossy().into_owned()
    };
    let write_prefs = |user_data: &Path, profile: &str, file: &str, settings: serde_json::Value| {
        let dir = user_data.join(profile);
        fs::create_dir_all(&dir).unwrap();
        let prefs = serde_json::json!({ "extensions": { "settings": settings } });
        fs::write(dir.join(file), prefs.to_string()).unwrap();
    };

    // 앱이 확장을 복사해 두는 곳 (BundledExtension::installed_dir)
    let ours = make_extension("ours", ChromeExtensionDetector::MANIFEST_NAME);
    let installed = PathBuf::from(&ours);
    let other = make_extension("other", "Other Extension");
    let browser = root.join("browser");

    assert_eq!(ChromeExtensionDetector::find_in(&browser, &installed), None, "프로필 없음");

    // 다른 확장 · 꺼진 확장 · 사라진 경로 · 웹스토어(상대 경로)는 감지하지 않는다
    write_prefs(
        &browser,
        "Default",
        "Secure Preferences",
        serde_json::json!({
            "aaa": { "path": other, "location": 4 },
            "bbb": { "path": ours, "location": 4, "disable_reasons": [1] },
            "ccc": { "path": root.join("gone").to_string_lossy(), "location": 4 },
            "ddd": { "path": "ddd/1.0_0", "location": 1 },
        }),
    );
    assert_eq!(ChromeExtensionDetector::find_in(&browser, &installed), None, "감지 대상 없음");

    // 다른 프로필의 Preferences 에 켜진 확장. 앱이 복사해 둔 곳이면 경로만 보고 안다
    write_prefs(&browser, "Profile 2", "Preferences", serde_json::json!({ "eee": { "path": ours, "location": 4, "disable_reasons": [] } }));
    assert_eq!(
        ChromeExtensionDetector::find_in(&browser, &installed),
        Some(FoundExtension { profile: "Profile 2".to_owned(), extension_id: "eee".to_owned(), path: ours.clone() }),
        "다른 프로필에서 감지"
    );

    // 다른 곳에서 로드한 LinkGroupz 확장: 브라우저가 기록한 권한이 맞을 때만 manifest.json 이름을 확인한다
    let elsewhere = make_extension("elsewhere", ChromeExtensionDetector::MANIFEST_NAME);
    let original = make_extension("original", "LinkGroups Tab Groups");
    let permissions = serde_json::json!({ "api": ["nativeMessaging", "tabs", "webNavigation", "tabGroups"] });
    let second = root.join("second-browser");
    write_prefs(
        &second,
        "Default",
        "Secure Preferences",
        serde_json::json!({
            "aaa": { "path": original, "active_permissions": permissions },
            "bbb": { "path": elsewhere },
            "ccc": { "path": elsewhere, "active_permissions": { "api": ["tabs"] } },
        }),
    );
    assert_eq!(ChromeExtensionDetector::find_in(&second, &installed), None, "원본 확장 · 권한이 다른 확장은 쓰지 않는다");
    write_prefs(&second, "Profile 1", "Preferences", serde_json::json!({ "fff": { "path": elsewhere, "granted_permissions": permissions } }));
    assert_eq!(
        ChromeExtensionDetector::find_in(&second, &installed).map(|found| found.extension_id),
        Some("fff".to_owned()),
        "다른 곳에서 로드한 LinkGroupz 확장"
    );

    assert!(!ChromeExtensionDetector::is_enabled(&serde_json::json!({ "state": 0 })), "예전 state 0 = 꺼짐");
    assert!(ChromeExtensionDetector::is_enabled(&serde_json::json!({ "state": 1 })), "예전 state 1 = 켜짐");
    assert!(ChromeExtensionDetector::is_enabled(&serde_json::json!({ "disable_reasons": 0 })), "disable_reasons 0 = 켜짐");
    assert!(ChromeExtensionDetector::is_enabled(&serde_json::json!({})), "기록 없음 = 켜짐");

    fs::create_dir_all(browser.join("Profile 10")).unwrap();
    fs::create_dir_all(browser.join("System Profile")).unwrap();
    assert_eq!(ChromeExtensionDetector::profile_names(&browser), ["Default", "Profile 2", "Profile 10"], "Default 먼저, 번호순");
}

/// macOS 가 권한을 묻는 폴더 안의 확장은 폴더를 열지 않는다 (문서 폴더 권한 창이 뜨지 않게).
#[test]
fn extension_folders_that_need_permission() {
    let home = dirs::home_dir().unwrap();
    let guarded = ChromeExtensionDetector::is_permission_guarded(&home.join("Documents/LinkGroups/chrome-extension"));
    assert_eq!(guarded, cfg!(target_os = "macos"), "문서 폴더 (권한은 macOS 만 묻는다)");
    assert_eq!(ChromeExtensionDetector::is_permission_guarded(&home.join("Desktop/ext")), cfg!(target_os = "macos"), "데스크탑");
    assert_eq!(ChromeExtensionDetector::is_permission_guarded(&home.join("Downloads/ext")), cfg!(target_os = "macos"), "다운로드");
    assert!(!ChromeExtensionDetector::is_permission_guarded(&BundledExtension::installed_dir()), "앱이 확장을 복사해 두는 곳");
    assert!(!ChromeExtensionDetector::is_permission_guarded(&home.join("dev/ext")), "그 밖의 폴더");
}

#[test]
fn group_open_mode() {
    let calls = Cell::new(0);
    let installed = || {
        calls.set(calls.get() + 1);
        true
    };
    assert!(GroupOpenMode::Auto.uses_tab_group(installed), "auto + 설치됨");
    assert!(!GroupOpenMode::Auto.uses_tab_group(|| false), "auto + 없음");
    assert!(GroupOpenMode::TabGroup.uses_tab_group(|| false), "항상 탭 그룹");
    assert!(!GroupOpenMode::NewWindow.uses_tab_group(installed), "항상 새 창");
    assert_eq!(calls.get(), 1, "auto 일 때만 감지");
    assert_eq!(serde_json::to_string(&GroupOpenMode::ALL).unwrap(), r#"["auto","tabGroup","newWindow"]"#, "설정에 적는 이름");
}

// MARK: - 브라우저 탭 가져오기

#[test]
fn tab_import_candidates() {
    let existing = [link("https://a.com/page/")];
    let tabs = [
        ChromeTab::new(1, "A", "https://a.com/page#top"),
        ChromeTab::new(1, "B", "https://b.com"),
        ChromeTab::new(1, "확장", "chrome://extensions/"),
        ChromeTab::new(2, "B 중복", "https://b.com/"),
        ChromeTab::new(2, "C", "https://c.com/x?y=1"),
    ];
    let candidates = TabImportCandidate::make(&tabs, &existing);
    let titles: Vec<&str> = candidates.iter().map(|candidate| candidate.tab.title.as_str()).collect();
    assert_eq!(titles, ["A", "B", "확장", "C"], "중복 URL 제거");
    let statuses: Vec<CandidateStatus> = candidates.iter().map(|candidate| candidate.status).collect();
    assert_eq!(
        statuses,
        [CandidateStatus::AlreadyInGroup, CandidateStatus::Available, CandidateStatus::Unsupported, CandidateStatus::Available],
        "상태"
    );
    assert_eq!(candidates.iter().map(|candidate| candidate.id).collect::<Vec<_>>(), [0, 1, 2, 3], "id 는 순서대로");
    assert_eq!(candidates[1].make_link(true).unwrap().alias.as_deref(), Some("B"), "제목을 alias 로");
    assert_eq!(candidates[1].make_link(false).unwrap().alias, None, "alias 없이");
    assert!(candidates[2].make_link(true).is_none(), "추가 불가 주소");
}

#[test]
fn tab_script_result_parse() {
    let list = ScriptValue::List(vec![
        ScriptValue::List(vec![ScriptValue::Int(2), ScriptValue::Text("제목".to_owned()), ScriptValue::Text("https://x.com".to_owned())]),
        ScriptValue::List(vec![ScriptValue::Text("only one".to_owned())]),
    ]);
    assert_eq!(ChromeTabsReader::parse(&list), [ChromeTab::new(2, "제목", "https://x.com")], "목록 파싱");
    assert!(ChromeTabsReader::parse(&ScriptValue::List(Vec::new())).is_empty(), "빈 목록");
    assert!(ChromeTabsReader::parse(&ScriptValue::Null).is_empty(), "목록이 아닌 값");

    let failure = |number| TabsError::from(ScriptFailure { number, message: Some("메시지".to_owned()) });
    assert_eq!(failure(-1743), TabsError::NotAuthorized, "자동화 권한 거부");
    assert_eq!(failure(9001), TabsError::NoWindow, "창 없음");
    assert_eq!(failure(-1728), TabsError::Script("메시지".to_owned()), "그 밖의 오류");
    assert_eq!(TabsError::from(ScriptFailure { number: -50, message: None }), TabsError::Script("오류 -50".to_owned()));
}

#[test]
fn link_drag_payload() {
    let group = Id::new();
    let link_ids = vec![Id::new(), Id::new()];
    let text = LinkDragPayload::new(group, link_ids.clone()).text();
    assert_eq!(LinkDragPayload::parse(&text), Some(LinkDragPayload::new(group, link_ids)), "왕복");
    assert_eq!(LinkDragPayload::parse("https://example.com"), None, "다른 문자열은 무시");
    assert_eq!(LinkDragPayload::parse(&format!("{}{group}|", LinkDragPayload::PREFIX)), None, "페이지 없음");
}

// MARK: - 브라우저 스냅샷 (확장 → 앱)

#[test]
fn snapshot_decode_and_order() {
    // 확장이 만드는 JSON 과 같은 모양
    let json = r#"{"nonce":"N1","windows":[
      {"id":10,"focused":false,"tabs":[{"title":"뒤 창","url":"https://back.com","groupId":-1}]},
      {"id":20,"focused":true,"tabs":[
        {"title":"A","url":"https://a.com","groupId":7},
        {"title":"B","url":"https://b.com","groupId":7},
        {"title":"설정","url":"chrome://settings","groupId":7},
        {"title":"혼자","url":"https://solo.com","groupId":-1},
        {"title":"C","url":"https://c.com","groupId":8}
      ]}
    ],"groups":[
      {"id":7,"title":"업무","color":"purple","windowId":20},
      {"id":8,"title":"","color":"없는색","windowId":20},
      {"id":9,"title":"빈 그룹","color":"red","windowId":20}
    ]}"#;
    let url = ChromeSnapshot::result_prefix() + &Base64Url::encode(json.as_bytes());
    assert!(ChromeSnapshot::decode(&url, "다른요청").is_none(), "nonce 불일치");
    assert!(ChromeSnapshot::decode(&ChromeSnapshot::snapshot_url("N1"), "N1").is_none(), "아직 결과가 아님");
    let snapshot = ChromeSnapshot::decode(&url, "N1").expect("디코드");

    let tabs = snapshot.tabs();
    assert_eq!(tabs.iter().map(|tab| tab.window_index).collect::<Vec<_>>(), [1, 1, 1, 1, 1, 2], "맨 앞 창이 창 1");
    assert_eq!(tabs[0].tab_group, Some(ChromeTabGroupInfo::new(7, "업무", GroupColor::Purple)), "탭 그룹 정보");
    assert_eq!(tabs[3].tab_group, None, "그룹 없는 탭");
    let unnamed = tabs[4].tab_group.as_ref().unwrap();
    assert!(unnamed.color == GroupColor::Grey && unnamed.display_title() == "이름 없는 그룹", "모르는 색 · 빈 이름");

    let groups = snapshot.tab_groups();
    assert_eq!(groups.iter().map(ChromeTabGroupImport::id).collect::<Vec<_>>(), [7, 8], "탭 있는 그룹만, 탭 순서대로");
    let made = groups[0].make_link_group(true, None);
    assert!(made.name == "업무" && made.color == GroupColor::Purple, "이름 · 색");
    assert_eq!(aliases(&made.links), [Some("A"), Some("B")], "열 수 없는 주소는 제외");
    assert_eq!(groups[1].make_link_group(false, None).links[0].alias, None, "alias 없이");
    let statuses: Vec<CandidateStatus> = groups[0].candidates(None).iter().map(|candidate| candidate.status).collect();
    assert_eq!(statuses, [CandidateStatus::Available, CandidateStatus::Available, CandidateStatus::Unsupported], "그룹 안 후보");
    assert_eq!(aliases(&groups[0].make_link_group(true, Some(&HashSet::from([1]))).links), [Some("B")], "고른 페이지만");
    assert!(groups[0].make_link_group(true, Some(&HashSet::from([2]))).links.is_empty(), "열 수 없는 주소는 골라도 제외");

    let candidates = TabImportCandidate::make(&tabs, &[]);
    assert_eq!(candidates[0].tab.tab_group.as_ref().map(|info| info.id), Some(7), "후보에 탭 그룹 유지");
}

#[test]
fn tab_group_import_plan() {
    let existing = LinkGroup::new("업무 ").with_color(GroupColor::Green).with_links(vec![link("https://a.com/")]);
    let tab_group = |id: i64, title: &str, color: GroupColor, window: usize, tabs: &[(&str, &str)]| ChromeTabGroupImport {
        info: ChromeTabGroupInfo::new(id, title, color),
        window_index: window,
        tabs: tabs.iter().map(|(title, url)| ChromeTab::new(window, *title, *url)).collect(),
    };
    let work1 = tab_group(1, "업무", GroupColor::Blue, 1, &[("A", "https://a.com"), ("B", "https://b.com")]);
    let work2 = tab_group(2, "업무", GroupColor::Red, 2, &[("B 다른 창", "https://b.com/"), ("C", "https://c.com")]);
    let news1 = tab_group(3, "뉴스", GroupColor::Pink, 1, &[("N1", "https://n1.com")]);
    let news2 = tab_group(4, "뉴스", GroupColor::Cyan, 2, &[("N1 again", "https://n1.com"), ("N2", "https://n2.com")]);
    let existing_groups = [existing.clone()];

    assert_eq!(work1.matching_group(&existing_groups).map(|group| group.id), Some(existing.id), "이름 공백 · 대소문자 무시 매칭");
    let statuses: Vec<CandidateStatus> = work1.candidates(Some(&existing)).iter().map(|candidate| candidate.status).collect();
    assert_eq!(statuses, [CandidateStatus::AlreadyInGroup, CandidateStatus::Available], "기존 그룹에 있는 페이지는 이미 있음");

    let selections: HashMap<i64, HashSet<usize>> = HashMap::from([
        (1, HashSet::from([0, 1])),
        (2, HashSet::from([0, 1])),
        (3, HashSet::from([0])),
        (4, HashSet::from([0, 1])),
    ]);
    let all = [work1.clone(), work2, news1.clone(), news2];
    let plan = TabGroupImportPlan::make(&all, &selections, &existing_groups, true, Browser::DEFAULT);
    assert!(plan.additions.len() == 1 && plan.additions[0].group_id == existing.id, "같은 이름 → 기존 그룹에 추가");
    assert_eq!(aliases(&plan.additions[0].links), [Some("B"), Some("C")], "이미 있는 A 제외, 다른 창의 같은 B 한 번만");
    assert_eq!(names(&plan.new_groups), ["뉴스"], "같은 이름 탭 그룹은 새 그룹 하나로");
    assert_eq!(plan.new_groups[0].color, GroupColor::Pink, "먼저 나온 탭 그룹의 색");
    assert_eq!(aliases(&plan.new_groups[0].links), [Some("N1"), Some("N2")], "새 그룹 안 중복 URL 제거");
    assert_eq!(plan.page_count(), 4, "페이지 수");
    assert_eq!(plan.new_groups[0].browser, Browser::Chrome, "브라우저를 안 고르면 새 그룹은 Chrome");

    let edge_plan =
        TabGroupImportPlan::make(&[news1], &HashMap::from([(3, HashSet::from([0]))]), &[], true, Browser::Edge);
    assert_eq!(edge_plan.new_groups[0].browser, Browser::Edge, "Edge 에서 가져온 새 그룹은 Edge 에서 열림");

    let none = TabGroupImportPlan::make(&[work1], &HashMap::from([(1, HashSet::from([0]))]), &existing_groups, true, Browser::DEFAULT);
    assert!(none.is_empty(), "이미 있는 페이지만 고르면 아무것도 안 함");
}

#[test]
fn native_messaging() {
    // 길이(4바이트 LE) + JSON 왕복, 연속 메시지
    let first = br#"{"a":1}"#.to_vec();
    let second = "{\"한글\":\"값\"}".as_bytes().to_vec();
    let mut pipe = Cursor::new([NativeMessaging::encode(&first), NativeMessaging::encode(&second)].concat());
    assert_eq!(NativeMessaging::read_message(&mut pipe), Some(first), "첫 메시지");
    assert_eq!(NativeMessaging::read_message(&mut pipe), Some(second), "둘째 메시지");
    assert_eq!(NativeMessaging::read_message(&mut pipe), None, "끝");
    assert_eq!(NativeMessaging::encode(&[0, 0, 0])[..4], [3, 0, 0, 0], "little endian 길이");
    assert_eq!(NativeMessaging::read_message(&mut Cursor::new(vec![5, 0, 0, 0, b'{'])), None, "잘린 메시지");

    let scratch = Scratch::new();
    let dir = scratch.path().join("native");
    let file = dir.join("chrome-snapshot.json");
    let edge_file = dir.join("edge-snapshot.json");
    let launch = Some(Timestamp::from_unix_seconds(1_000_000.0));
    // 1.2 확장은 browser 를 보내지 않는다 → Chrome 으로 본다
    let fresh = r#"{"savedAt":1000001000,"windows":[{"id":1,"focused":true,"tabs":[{"title":"T","url":"https://t.com","groupId":-1}]}],"groups":[]}"#;
    let stale = r#"{"savedAt":999999000,"windows":[],"groups":[]}"#;
    let edge = r#"{"browser":"edge","savedAt":1000001000,"windows":[{"id":1,"focused":true,"tabs":[{"title":"E","url":"https://e.com","groupId":-1}]}],"groups":[]}"#;
    let titles = |file: &Path| -> Option<Vec<String>> {
        NativeSnapshotStore::read_fresh(file, launch).map(|snapshot| snapshot.tabs().into_iter().map(|tab| tab.title).collect())
    };

    assert!(NativeSnapshotStore::write(b"not a snapshot", &dir).is_err(), "스냅샷이 아닌 메시지는 쓰지 않음");
    assert!(!file.exists());

    assert_eq!(NativeSnapshotStore::write(fresh.as_bytes(), &dir).unwrap(), file, "browser 없는 스냅샷은 chrome-snapshot.json");
    assert_eq!(titles(&file), Some(vec!["T".to_owned()]), "브라우저 실행 뒤 저장본 사용");
    assert_eq!(NativeSnapshotStore::write(edge.as_bytes(), &dir).unwrap(), edge_file, "Edge 스냅샷은 edge-snapshot.json");
    assert_eq!(titles(&edge_file), Some(vec!["E".to_owned()]), "Edge 저장본");
    assert_eq!(titles(&file), Some(vec!["T".to_owned()]), "Edge 가 Chrome 저장본을 덮지 않음");
    NativeSnapshotStore::write(stale.as_bytes(), &dir).unwrap();
    assert_eq!(titles(&file), None, "브라우저 실행 전 저장본은 버림");
    assert!(NativeSnapshotStore::read_fresh(&file, None).is_some(), "켜진 시각을 모르면 저장본을 쓴다");
    assert_eq!(titles(&dir.join("none.json")), None, "파일 없음");

    let manifest = dir.join("NativeMessagingHosts").join(format!("{}.json", NativeSnapshotStore::HOST_NAME));
    assert!(NativeSnapshotStore::install_manifest("abc", "/x/Host", &manifest).unwrap(), "처음 설치");
    assert!(!NativeSnapshotStore::install_manifest("abc", "/x/Host", &manifest).unwrap(), "같은 내용이면 다시 쓰지 않음");
    assert!(NativeSnapshotStore::install_manifest("def", "/x/Host", &manifest).unwrap(), "확장 ID 바뀌면 다시 씀");
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    assert!(json["name"] == NativeSnapshotStore::HOST_NAME && json["type"] == "stdio", "호스트 이름 · 형식");
    assert_eq!(json["allowed_origins"], serde_json::json!(["chrome-extension://def/"]), "허용 확장");
    assert_eq!(json["path"], "/x/Host", "호스트 경로");
}

// MARK: - 쿠키

#[test]
fn stored_cookie() {
    let now = Timestamp::now();
    let future = now.plus_seconds(3600.0);
    let mut cookie = StoredCookie::new("SID", "abc", ".example.com").with_expires(future);
    cookie.path = "/app".to_owned();
    cookie.secure = true;
    cookie.http_only = true;
    cookie.same_site = Some("lax".to_owned());

    let json = serde_json::to_value(&cookie).unwrap();
    assert!(json["httpOnly"] == true && json["sameSite"] == "lax" && json["path"] == "/app", "Swift 판과 같은 키");
    assert_eq!(serde_json::from_value::<StoredCookie>(json).unwrap(), cookie, "왕복");
    assert_eq!(cookie.site(), "example.com", "도메인 앞의 . 은 뗀다");

    let session = StoredCookie::new("S", "v", "a.com");
    assert!(session.expires.is_none() && !serde_json::to_string(&session).unwrap().contains("expires"), "세션 쿠키");
    assert!(StoredCookie::new("S", "v", "a.com").with_expires(Timestamp::from_unix_seconds(1.0)).is_expired(now), "만료됨");
    assert!(!session.is_expired(now), "세션 쿠키는 만료 아님");
    assert!(!cookie.is_expired(now), "아직 만료 전");
}

#[cfg(any(target_os = "macos", windows))]
#[test]
fn keychain_cookie_vault() {
    // 실제 로그인 키체인(Windows 는 자격 증명 관리자)에 검사용 항목을 만들었다가 지운다.
    let vault = CookieVault::new(&format!("dev.linkgroupz.checks-{}", Id::new()));
    let now = Timestamp::now();
    let names = |vault: &CookieVault| -> Vec<String> { vault.load().unwrap().into_iter().map(|cookie| cookie.name).collect() };

    let result = (|| -> Result<(), CookieVaultError> {
        assert!(vault.load()?.is_empty(), "처음엔 비어 있음");
        let live = StoredCookie::new("A", "1", "a.com").with_expires(now.plus_seconds(600.0));
        let session = StoredCookie::new("B", "2", "b.com");
        let expired = StoredCookie::new("C", "3", "c.com").with_expires(Timestamp::from_unix_seconds(1.0));
        assert!(vault.save(&[session.clone(), live.clone(), expired], now)?, "처음 저장");
        assert_eq!(names(&vault), ["A", "B"], "만료 쿠키는 빼고 저장");
        assert!(!vault.save(&[live.clone(), session], now)?, "같은 내용이면 다시 쓰지 않음");
        vault.save(&[live], now)?;
        assert_eq!(names(&vault), ["A"], "바뀐 내용으로 갱신");
        vault.clear()?;
        assert!(vault.load()?.is_empty(), "지우기");
        Ok(())
    })();
    let _ = vault.clear();
    result.expect("키체인");
}

// MARK: - 확장 · 아이콘

#[test]
fn bundled_extension_sync() {
    let scratch = Scratch::new();
    let source = scratch.path().join("bundle").join("chrome-extension");
    let target = scratch.path().join("data").join("chrome-extension");
    fs::create_dir_all(source.join("icons")).unwrap();
    let write_manifest = |version: &str| {
        fs::write(source.join("manifest.json"), serde_json::json!({ "name": "x", "version": version }).to_string()).unwrap();
    };
    write_manifest("1.0.0");
    fs::write(source.join("background.js"), "a").unwrap();
    fs::write(source.join("icons").join("16.png"), "png").unwrap();

    assert_eq!(BundledExtension::sync(Some(&source), &target), Some(target.clone()), "처음 복사");
    assert_eq!(BundledExtension::version(&target).as_deref(), Some("1.0.0"), "버전");
    assert!(target.join("icons").join("16.png").exists(), "하위 폴더까지 복사");
    fs::write(target.join("note.txt"), "사용자 수정").unwrap();
    BundledExtension::sync(Some(&source), &target);
    assert!(target.join("note.txt").exists(), "같은 버전이면 그대로 둠");
    write_manifest("1.1.0");
    BundledExtension::sync(Some(&source), &target);
    assert_eq!(BundledExtension::version(&target).as_deref(), Some("1.1.0"), "버전 바뀌면 다시 복사");
    assert!(!target.join("note.txt").exists(), "이전 폴더 교체");
    assert_eq!(BundledExtension::sync(None, &target), None, "앱에 확장 없음");
}

#[test]
fn favicon_parser() {
    let page = "https://wiki.example.com:8443/pages/view?id=1#top";
    assert_eq!(FaviconParser::cache_key(page).as_deref(), Some("wiki.example.com_8443"), "포트 포함 캐시 키");
    assert_eq!(FaviconParser::cache_key("https://A.com/x").as_deref(), Some("a.com"), "소문자");
    assert_eq!(FaviconParser::default_icon_url(page).as_deref(), Some("https://wiki.example.com:8443/favicon.ico"), "루트 favicon.ico");
    assert_eq!(FaviconParser::default_icon_url("file:///tmp/a.html"), None, "파일 주소는 없음");
    assert_eq!(
        FaviconParser::default_icon_url("https://user:pw@a.com/x?y=1").as_deref(),
        Some("https://a.com/favicon.ico"),
        "계정 정보는 뗀다"
    );

    let html = r#"<html><head>
    <link rel="stylesheet" href="/a.css">
    <link rel="apple-touch-icon" href="/touch.png">
    <LINK REL='shortcut icon' HREF='/static/fav.ico'>
    <link href="https://cdn.example.com/icon.svg" rel="icon" type="image/svg+xml">
    <link rel=icon href=rel.png>
    </head></html>"#;
    assert_eq!(
        FaviconParser::icon_urls(html, "https://wiki.example.com/pages/"),
        [
            "https://wiki.example.com/static/fav.ico",
            "https://cdn.example.com/icon.svg",
            "https://wiki.example.com/pages/rel.png",
            "https://wiki.example.com/touch.png",
        ],
        "icon 우선, 상대 경로 해석"
    );
}

#[test]
fn favicon_known_paths() {
    let urls = FaviconParser::known_icon_urls("https://wiki.example.com:8443/pages/view?id=1#x");
    assert_eq!(urls[0], "https://wiki.example.com:8443/images/fav-jsw.png", "Jira 아이콘 먼저");
    assert!(urls.iter().any(|url| url == "https://wiki.example.com:8443/images/logo/confluence_48_trans.png"), "Confluence 아이콘");
    assert!(urls.iter().all(|url| !url.contains('?') && !url.contains('#')), "쿼리 · 조각 제거");
    assert!(FaviconParser::known_icon_urls("file:///tmp/a").is_empty(), "파일 주소는 없음");
}

#[test]
fn group_colors() {
    assert_eq!(GroupColor::ALL.len(), 19, "색 19가지");
    let chrome: Vec<&str> = GroupColor::ALL.iter().filter(|color| color.is_chrome_color()).map(|color| color.raw_value()).collect();
    assert_eq!(chrome, ["grey", "blue", "red", "yellow", "green", "pink", "purple", "cyan", "orange"], "Chrome 색 9가지");
    assert!(GroupColor::ALL.iter().all(|color| color.chrome_color().is_chrome_color()), "모든 색이 Chrome 색으로 대응");
    assert!(GroupColor::Navy.chrome_color() == GroupColor::Blue && GroupColor::Coral.chrome_color() == GroupColor::Red, "가까운 색");
    for color in GroupColor::ALL {
        assert_eq!(serde_json::to_string(&color).unwrap(), format!("\"{}\"", color.raw_value()), "파일에 적는 이름");
    }

    let decoded: Vec<GroupColor> = serde_json::from_str(r#"["navy","blue","rainbow"]"#).unwrap();
    assert_eq!(decoded, [GroupColor::Navy, GroupColor::Blue, GroupColor::Grey], "모르는 색 이름은 회색으로 읽음");

    let group = LinkGroup::new("남색 그룹").with_color(GroupColor::Navy).with_links(vec![link("https://a.com")]);
    assert_eq!(TabGroupMarker::decode(&TabGroupMarker::url(&group)).unwrap().color, "blue", "탭 그룹으로 열 때는 Chrome 색");
}

#[test]
fn browser() {
    assert_eq!(Browser::DEFAULT, Browser::Chrome, "기본은 Chrome");
    assert!(
        Browser::Chrome.bundle_identifier() == "com.google.Chrome" && Browser::Edge.bundle_identifier() == "com.microsoft.edgemac",
        "번들 ID"
    );
    assert_eq!(NativeSnapshotStore::file_path(Browser::Chrome).file_name().unwrap(), "chrome-snapshot.json", "Chrome 저장 파일 이름");
    assert!(ChromeTabsReader::source(Browser::Edge).contains("application id \"com.microsoft.edgemac\""), "Edge AppleScript 대상");

    #[cfg(target_os = "macos")]
    {
        assert!(Browser::Edge.user_data_directory().ends_with("Library/Application Support/Microsoft Edge"), "Edge 프로필 폴더");
        assert!(Browser::Chrome.user_data_directory().ends_with("Library/Application Support/Google/Chrome"), "Chrome 프로필 폴더");
        let manifest = format!("Microsoft Edge/NativeMessagingHosts/{}.json", NativeSnapshotStore::HOST_NAME);
        assert!(NativeSnapshotStore::manifest_path(Browser::Edge).ends_with(manifest), "Edge 호스트 등록 위치");
    }
    #[cfg(windows)]
    {
        assert!(Browser::Edge.user_data_directory().ends_with("Microsoft\\Edge\\User Data"), "Edge 프로필 폴더");
        assert!(Browser::Chrome.user_data_directory().ends_with("Google\\Chrome\\User Data"), "Chrome 프로필 폴더");
        assert!(NativeSnapshotStore::manifest_path(Browser::Edge).ends_with("native-host\\edge.json"), "Edge 호스트 등록 파일");
    }

    // 1.2 이하 data.json 에는 browser 가 없다 → Chrome
    let decoded: LinkGroup = serde_json::from_str(LEGACY_GROUP).unwrap();
    assert_eq!(decoded.browser, Browser::Chrome, "browser 없는 그룹은 Chrome");
    let mut group = decoded;
    group.browser = Browser::Edge;
    let round_trip: LinkGroup = serde_json::from_str(&serde_json::to_string(&group).unwrap()).unwrap();
    assert_eq!(round_trip.browser, Browser::Edge, "browser 저장 · 복원");
    let unknown = LEGACY_GROUP.replace(r#""links":[]"#, r#""links":[],"browser":"safari""#);
    assert_eq!(serde_json::from_str::<LinkGroup>(&unknown).unwrap().browser, Browser::Chrome, "모르는 브라우저 값은 Chrome");

    let marker = ChromeSnapshot::result_prefix()
        + &Base64Url::encode(br#"{"nonce":"N","browser":"edge","windows":[],"groups":[]}"#);
    assert_eq!(ChromeSnapshot::decode(&marker, "N").unwrap().source_browser(), Browser::Edge, "마커 결과의 browser");
}

#[test]
fn group_icon() {
    let old: LinkGroup = serde_json::from_str(LEGACY_GROUP).unwrap();
    assert_eq!(old.icon, GroupIcon::default(), "icon 없는 그룹은 기본 폴더");
    let broken = LEGACY_GROUP.replace(r#""links":[]"#, r#""links":[],"icon":{"what":1}"#);
    assert_eq!(serde_json::from_str::<LinkGroup>(&broken).unwrap().icon, GroupIcon::default(), "읽을 수 없는 icon 은 기본 폴더 (그룹은 살림)");
    for icon in [GroupIcon::Symbol("server.rack".to_owned()), GroupIcon::Image("A.png".to_owned())] {
        let mut group = old.clone();
        group.icon = icon.clone();
        let decoded: LinkGroup = serde_json::from_str(&serde_json::to_string(&group).unwrap()).unwrap();
        assert_eq!(decoded.icon, icon, "icon 저장 · 복원");
    }
    assert_eq!(serde_json::to_string(&GroupIcon::default()).unwrap(), r#"{"symbol":"folder.fill"}"#, "파일에 적는 모양");
    assert_eq!(serde_json::to_string(&GroupIcon::Image("A.png".to_owned())).unwrap(), r#"{"image":"A.png"}"#);

    let presets = GroupIcon::PRESET_SYMBOLS;
    assert!(presets.len() == 40 && presets.iter().collect::<HashSet<_>>().len() == 40, "기본 아이콘 40개, 중복 없음");
    assert_eq!(presets[0], GroupIcon::DEFAULT_SYMBOL, "첫 아이콘이 기본 폴더");
    for removed in ["music.note", "cart.fill", "cup.and.saucer.fill", "paintpalette.fill"] {
        assert!(!presets.contains(&removed), "뺀 아이콘: {removed}");
    }
    for added in ["display", "cylinder.split.1x2.fill", "chart.xyaxis.line"] {
        assert!(presets.contains(&added), "넣은 아이콘: {added}");
    }

    let scratch = Scratch::new();
    let dir = scratch.path();
    assert_eq!(GroupIconStore::file_path("../data.json", dir), None, "상위 폴더 이름 거부");
    assert_eq!(GroupIconStore::file_path("a/b.png", dir), None, "경로 이름 거부");
    assert_eq!(GroupIconStore::file_path("a\\b.png", dir), None, "Windows 경로 이름 거부");
    assert_eq!(GroupIconStore::file_path(".hidden.png", dir), None, "숨김 파일 거부");
    assert_eq!(GroupIconStore::file_path("", dir), None, "빈 이름 거부");
    assert_eq!(GroupIconStore::file_path("ok.png", dir).unwrap().parent().unwrap().file_name().unwrap(), "icons", "icons 폴더 안");

    // 가로로 긴 이미지 → 128x128 PNG
    let source = dir.join("wide.png");
    image::RgbaImage::from_pixel(300, 100, image::Rgba([200, 40, 40, 255])).save(&source).unwrap();
    let name = GroupIconStore::import_image(&source, dir).unwrap();
    let stored = image::open(GroupIconStore::file_path(&name, dir).unwrap()).unwrap();
    assert!(stored.width() == 128 && stored.height() == 128, "정사각형 128px 로 저장");
    assert!(GroupIconStore::import_image(&dir.join("none.txt"), dir).is_err(), "이미지가 아니면 거부");
    fs::write(dir.join("text.png"), "그림이 아님").unwrap();
    assert!(GroupIconStore::import_image(&dir.join("text.png"), dir).is_err(), "내용이 이미지가 아니면 거부");

    let other = GroupIconStore::import_image(&source, dir).unwrap();
    let mut user = old;
    user.icon = GroupIcon::Image(name.clone());
    assert_eq!(GroupIconStore::remove_unused(&[user], dir), [other], "쓰지 않는 이미지만 지움");
    assert!(GroupIconStore::file_path(&name, dir).unwrap().exists(), "쓰는 이미지는 남김");
}

// MARK: - 업데이트

#[test]
fn update_rules() {
    let v = |text: &str| AppVersion::parse(text).unwrap();
    assert!(v("1.4.1") > v("1.4.0"), "1.4.1 > 1.4.0");
    assert!(v("1.10.0") > v("1.9.9"), "숫자로 비교 (1.10 > 1.9)");
    assert_eq!(v("1.4"), v("1.4.0"), "없는 칸은 0");
    assert!(v("2.0.0") > v("1.99"), "주 버전");
    assert!(v("1.0.1") > v("1"), "모자란 칸은 0");
    assert!(AppVersion::parse("1.4.a").is_none() && AppVersion::parse("").is_none() && AppVersion::parse("1..2").is_none(), "버전이 아니면 None");
    assert_eq!(v("1.10.0").to_string(), "1.10.0");
    assert_eq!(HashSet::from([v("1.4"), v("1.4.0"), v("1.4.0.0")]).len(), 1, "같은 버전은 해시도 같음");

    let source = UpdateSource::new(" someone/some-releases ", " KEY== ").unwrap();
    assert_eq!(source.repository(), "someone/some-releases", "앞뒤 공백은 뗀다");
    assert_eq!(source.public_key(), "KEY==");
    assert_eq!(source.manifest_url(), "https://github.com/someone/some-releases/releases/latest/download/latest.json", "새 버전을 확인하는 주소");
    assert_eq!(source.releases_page_url(), "https://github.com/someone/some-releases/releases");
    assert_eq!(source.download_url("v1.5.5", "a.exe"), "https://github.com/someone/some-releases/releases/download/v1.5.5/a.exe");
    for bad in ["", "someone", "someone/", "/name", "a/b/c", "a/..", "https://github.com/a/b", "a b/c"] {
        assert!(UpdateSource::new(bad, "KEY").is_none(), "저장소 이름이 아님: {bad}");
    }
    assert!(UpdateSource::new("a/b", "  ").is_none(), "공개 키 없음");
    let settings = UpdateSource::parse_settings("# 설명\nLINKGROUPZ_RELEASES_REPO = a/b\n\nLINKGROUPZ_UPDATE_PUBKEY=\"K=\"\n잘못된 줄\n");
    assert_eq!(settings.get("LINKGROUPZ_RELEASES_REPO").map(String::as_str), Some("a/b"), "설정 파일: 이름=값");
    assert_eq!(settings.get("LINKGROUPZ_UPDATE_PUBKEY").map(String::as_str), Some("K="), "값의 따옴표는 떼고, 값 안의 = 는 남긴다");
    assert_eq!(settings.len(), 2, "설명 · 빈 줄 · 잘못된 줄은 건너뛴다");

    assert!(UpdatePromptPolicy::should_prompt(&v("1.5.0"), &v("1.4.1"), None), "새 버전이면 묻기");
    assert!(!UpdatePromptPolicy::should_prompt(&v("1.4.1"), &v("1.4.1"), None), "같은 버전이면 안 물음");
    assert!(!UpdatePromptPolicy::should_prompt(&v("1.5.0"), &v("1.4.1"), Some(&v("1.5.0"))), "나중에 고른 버전은 안 물음");
    assert!(UpdatePromptPolicy::should_prompt(&v("1.5.1"), &v("1.4.1"), Some(&v("1.5.0"))), "더 새 버전이 오면 다시 물음");
}

#[cfg(any(target_os = "macos", windows))]
#[test]
fn secret_store() {
    // 키체인(Windows 는 자격 증명 관리자): 시험용 서비스 이름으로 저장 · 읽기 · 지우기
    let store = SecretStore::new(format!("dev.linkgroupz.checks-{}", Id::new()), "checks", "LinkGroupz 시험");
    let result = (|| -> Result<(), SecretError> {
        assert_eq!(store.load()?, None, "처음엔 없음");
        store.save(b"secret-1")?;
        store.save(b"secret-2")?;
        assert_eq!(store.load()?.as_deref(), Some(&b"secret-2"[..]), "저장 · 덮어쓰기");
        store.delete()?;
        assert_eq!(store.load()?, None, "지우기");
        store.delete()?;
        Ok(())
    })();
    let _ = store.delete();
    result.expect("키체인");
}

#[test]
fn private_window() {
    let links = [link("https://a.com"), link("https://b.com/x?y=1")];
    assert_eq!(ChromeLauncher::private_window_arguments(&links, Browser::Chrome), ["--incognito", "https://a.com", "https://b.com/x?y=1"], "Chrome 시크릿");
    assert_eq!(ChromeLauncher::private_window_arguments(&links, Browser::Edge), ["--inprivate", "https://a.com", "https://b.com/x?y=1"], "Edge InPrivate");
    assert_eq!(ChromeLauncher::new_window_arguments(&links), ["--new-window", "https://a.com", "https://b.com/x?y=1"], "새 창");
}

/// 저장된 글자가 그대로 브라우저 실행 인자가 되면 data.json 한 줄로 브라우저 옵션을 줄 수 있다.
#[test]
fn only_openable_urls_reach_the_browser() {
    let evil = link("--renderer-cmd-prefix=touch /tmp/x");
    let good = link("https://a.com");
    assert_eq!(ChromeLauncher::new_window_arguments(&[good.clone(), evil.clone()]), ["--new-window", "https://a.com"], "새 창");
    assert_eq!(ChromeLauncher::private_window_arguments(std::slice::from_ref(&evil), Browser::Chrome), ["--incognito"], "시크릿 창");

    // 열 수 있는 주소가 하나도 없으면 브라우저를 찾기 전에 막는다
    assert!(matches!(ChromeLauncher::open_link(&evil, Browser::Chrome), Err(LauncherError::UnsupportedUrl(_))));
    let group = LinkGroup::new("x").with_links(vec![evil.clone()]);
    assert!(matches!(ChromeLauncher::open_group(&group, false), Err(LauncherError::UnsupportedUrl(_))), "그룹 열기");
    assert!(matches!(ChromeLauncher::open_private(&[evil], Browser::Edge), Err(LauncherError::UnsupportedUrl(_))), "시크릿 열기");
    assert!(TabGroupMarker::decode(&TabGroupMarker::url(&group)).unwrap().urls.is_empty(), "마커에도 싣지 않는다");

    for ok in ["http://a.com", "https://a.com/x?y=1", "file:///tmp/a.html"] {
        assert!(LinkUrl::new(ok).is_openable(), "{ok}");
    }
    for bad in ["-n", "--x", "javascript:alert(1)", "chrome://settings", "about:blank", "data:text/html,x", "", "a.com"] {
        assert!(!LinkUrl::new(bad).is_openable(), "{bad}");
    }
}

#[test]
fn release_notes() {
    let v = |text: &str| AppVersion::parse(text).unwrap();
    let versions = |entries: &[ReleaseEntry]| -> Vec<AppVersion> { entries.iter().map(|entry| entry.version.clone()).collect() };
    let real = ReleaseNotes::entries();
    let mut sorted = versions(real);
    sorted.sort_by(|a, b| b.cmp(a));
    sorted.dedup();
    assert_eq!(versions(real), sorted, "최신이 위, 버전 중복 없음");
    assert!(real.iter().all(|entry| !entry.changes.is_empty()), "모든 버전에 바뀐 점");
    assert!(real.iter().all(|entry| entry.changes_en.len() == entry.changes.len()), "모든 버전에 영어 바뀐 점 (한국어와 같은 수)");

    // 고르는 규칙은 예시 목록으로 본다.
    let all: Vec<ReleaseEntry> = ["1.5.4", "1.5.3", "1.5.2", "1.5.1"].iter().map(|version| ReleaseEntry { version: v(version), changes: vec!["바뀜".to_owned()], changes_en: vec!["changed".to_owned()] }).collect();
    let all = all.as_slice();
    assert_eq!(versions(&ReleaseNotes::entries_after(Some(&v("1.5.1")), &v("1.5.3"), all)), [v("1.5.3"), v("1.5.2")], "지난 버전 다음부터 지금까지");
    assert!(ReleaseNotes::entries_to_show(Some(&v("1.5.3")), true, &v("1.5.3"), all).is_empty(), "같은 버전이면 안 보여줌");
    assert_eq!(versions(&ReleaseNotes::entries_to_show(Some(&v("1.5.2")), true, &v("1.5.3"), all)), [v("1.5.3")], "업데이트하면 보여줌");
    assert!(ReleaseNotes::entries_to_show(None, false, &v("1.5.3"), all).is_empty(), "처음 설치는 안 보여줌 (시작 안내)");
    assert_eq!(versions(&ReleaseNotes::entries_to_show(None, true, &v("1.5.3"), all)), [v("1.5.3")], "기록 없는 기존 사용자는 지금 버전만");
    assert!(ReleaseNotes::entries_to_show(Some(&v("1.5.4")), true, &v("1.5.3"), all).is_empty(), "낮은 버전으로 되돌리면 안 보여줌");
    let package_version = v(env!("CARGO_PKG_VERSION"));
    assert!(real.iter().any(|entry| entry.version == package_version), "지금 버전(Cargo.toml)에 바뀐 점이 있음");
}

#[test]
fn always_private() {
    let old: LinkGroup = serde_json::from_str(LEGACY_GROUP).unwrap();
    assert!(!old.always_private, "값이 없으면 꺼짐");
    let mut group = old;
    group.always_private = true;
    let json = serde_json::to_string(&group).unwrap();
    assert!(json.contains(r#""alwaysPrivate":true"#), "파일에 적는 키");
    assert!(serde_json::from_str::<LinkGroup>(&json).unwrap().always_private, "저장 · 복원");
}

#[test]
fn base64url() {
    let data = "한글 / ? + = 테스트".as_bytes();
    let encoded = Base64Url::encode(data);
    assert!(!encoded.contains('+') && !encoded.contains('/') && !encoded.contains('='), "URL 안전 문자");
    assert_eq!(Base64Url::decode(&encoded).as_deref(), Some(data), "왕복");
    assert_eq!(Base64Url::decode("!!!"), None, "base64 가 아님");
}

// MARK: - 이름 (원본 LinkGroups 와 나란히 쓸 수 있게 모두 달라야 한다)

#[test]
fn brand_is_distinct_from_original() {
    assert_eq!(TabGroupMarker::HOST, "linkgroupz.invalid");
    assert!(TabGroupMarker::HOST.ends_with(".invalid"), "접속되지 않는 예약 도메인");
    assert_eq!(NativeSnapshotStore::HOST_NAME, "dev.linkgroupz.snapshot");
    assert_eq!(ChromeExtensionDetector::MANIFEST_NAME, "LinkGroupz Tab Groups");
    for name in [
        brand::APP_NAME,
        brand::APP_IDENTIFIER,
        brand::DATA_DIR_NAME,
        brand::MARKER_HOST,
        brand::NATIVE_HOST_NAME,
        brand::EXTENSION_MANIFEST_NAME,
        brand::COOKIE_VAULT_SERVICE,
    ] {
        assert!(!name.to_lowercase().contains("linkgroups"), "원본과 겹치는 이름: {name}");
    }
    assert!(ChromeSnapshot::push_url("N").starts_with("https://linkgroupz.invalid/push#"));
}

// MARK: - 섹션

/// 맨 위 단계와 섹션 안을 한 줄로: "A [S: B C] D"
fn outline(store: &LinkStore) -> String {
    let name = |id: Id| store.group(id).map(|group| group.name.clone()).unwrap_or_else(|| "?".to_owned());
    store
        .sidebar_order()
        .iter()
        .map(|item| match item {
            SidebarItem::Group(id) => name(*id),
            SidebarItem::Section(id) => {
                let section = store.section(*id).unwrap();
                let inside: Vec<String> = store.groups_in_section(*id).iter().map(|group| group.name.clone()).collect();
                format!("[{}: {}]", section.name, inside.join(" "))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn sections() {
    let scratch = Scratch::new();
    let mut store = LinkStore::open(scratch.data_file());
    let [a, b, c, d] = ["A", "B", "C", "D"].map(|name| store.add_group(LinkGroup::new(name)).id);
    assert_eq!(outline(&store), "A B C D");
    let saved = fs::read_to_string(scratch.data_file()).unwrap();
    assert!(!saved.contains("sections") && !saved.contains("sidebarOrder"), "섹션이 없으면 이전 형식 그대로");

    // 그룹 자리에 섹션을 만들고 그 그룹과 다른 그룹을 넣는다 (넣는 순서는 화면 순서)
    let s = store.add_section("  업무 ", Some(c), &[d, b]).id;
    assert_eq!(outline(&store), "A [업무: B D] C", "섹션은 C 자리에, 안은 화면 순서");
    assert_eq!(names(store.groups()), ["A", "B", "D", "C"], "그룹 목록은 화면 순서");

    // 섹션 안으로: 특정 그룹 앞 · 끝
    store.move_items(&[a], DropTarget { section: Some(s), before: Some(d), after: None });
    assert_eq!(outline(&store), "[업무: B A D] C", "섹션 안 D 앞");
    store.move_items(&[b], DropTarget::into_section(s));
    assert_eq!(outline(&store), "[업무: A D B] C", "섹션 끝");

    // 섹션 밖으로: 맨 위 단계 항목 앞 · 뒤 · 끝
    store.move_items(&[d], DropTarget { section: None, before: Some(s), after: None });
    assert_eq!(outline(&store), "D [업무: A B] C", "섹션 앞");
    store.move_items(&[a, d], DropTarget { section: None, before: None, after: Some(c) });
    assert_eq!(outline(&store), "[업무: B] C D A", "여러 개는 화면 순서대로, C 뒤");
    store.move_items(&[s], DropTarget::top_level_end());
    assert_eq!(outline(&store), "C D A [업무: B]", "섹션 옮기기");
    store.move_items(&[s], DropTarget::into_section(s));
    assert_eq!(outline(&store), "C D A [업무: B]", "자기 안으로는 못 넣는다");

    let t = store.add_section("빈 섹션", None, &[]).id;
    store.move_items(&[s], DropTarget::into_section(t));
    assert_eq!(outline(&store), "C D A [업무: B] [빈 섹션: ]", "섹션은 섹션 안에 넣지 못하고 그 앞에 놓인다");
    store.move_section(t, -1);
    assert_eq!(outline(&store), "C D A [빈 섹션: ] [업무: B]", "한 칸 위로");
    store.move_section(t, -10);
    assert_eq!(outline(&store), "[빈 섹션: ] C D A [업무: B]", "맨 위에서 멈춘다");

    store.rename_section(s, "회의");
    store.rename_section(s, "  ");
    store.set_section_collapsed(s, true);
    assert!(store.section(s).unwrap().is_collapsed && store.section(s).unwrap().name == "회의", "이름 · 접기 (빈 이름은 무시)");

    // 그룹 편집에서 섹션을 바꾸면: 넣을 때는 그 섹션 끝, 뺄 때는 섹션 바로 뒤
    let mut moved = store.group(c).unwrap().clone();
    moved.section_id = Some(s);
    store.update_group_keeping_place(moved);
    assert_eq!(outline(&store), "[빈 섹션: ] D A [회의: B C]");
    let mut moved = store.group(b).unwrap().clone();
    moved.section_id = None;
    store.update_group_keeping_place(moved);
    assert_eq!(outline(&store), "[빈 섹션: ] D A [회의: C] B");

    // 새 그룹은 끝, 섹션을 정해 만들면 그 섹션 끝
    let mut e = LinkGroup::new("E");
    e.section_id = Some(t);
    store.add_group(e);
    assert_eq!(outline(&store), "[빈 섹션: E] D A [회의: C] B");

    // 섹션을 지우면 안의 그룹은 그 자리에 남는다
    store.delete_section(t);
    assert_eq!(outline(&store), "E D A [회의: C] B");

    // 보관했다 되돌리면 섹션 밖 끝으로
    store.archive_groups(&ids([c]), Timestamp::now());
    assert_eq!(outline(&store), "E D A [회의: ] B", "빈 섹션은 남는다");
    store.restore_groups(&ids([c]));
    assert_eq!(outline(&store), "E D A [회의: ] B C");

    // 다시 읽어도 같다
    store.move_items(&[a], DropTarget::into_section(s));
    let reopened = LinkStore::open(scratch.data_file());
    assert_eq!(outline(&reopened), "E D [회의: A] B C", "저장 · 읽기");
    assert!(reopened.section(s).unwrap().is_collapsed);
}

/// 원본 LinkGroups 1.5.5 이후가 쓴 파일을 그대로 읽고, 같은 형식으로 쓴다.
#[test]
fn sections_file_format_matches_original() {
    let scratch = Scratch::new();
    let [s, g1, g2, g3] = [Id::new(), Id::new(), Id::new(), Id::new()];
    let group = |id: Id, name: &str, section: Option<Id>| {
        let mut value = serde_json::json!({"id": id.to_string(), "name": name, "color": "blue", "tags": [], "links": []});
        if let Some(section) = section {
            value["sectionID"] = serde_json::json!(section.to_string());
        }
        value
    };
    let file = serde_json::json!({
        "version": 1,
        // 그룹 목록 순서와 맨 위 순서가 달라도 맨 위 순서를 따른다
        "groups": [group(g1, "가", Some(s)), group(g2, "나", Some(s)), group(g3, "다", None)],
        "archive": {"groups": [], "links": []},
        "sections": [{"id": s.to_string(), "name": "섹션A", "isCollapsed": false}],
        "sidebarOrder": [format!("section:{s}"), format!("group:{g3}"), "알 수 없는 칸", format!("group:{}", Id::new())],
    });
    fs::create_dir_all(scratch.data_file().parent().unwrap()).unwrap();
    fs::write(scratch.data_file(), serde_json::to_vec(&file).unwrap()).unwrap();

    let mut store = LinkStore::open(scratch.data_file());
    assert!(store.last_error().is_none());
    assert_eq!(outline(&store), "[섹션A: 가 나] 다", "읽을 수 없는 칸 · 없는 그룹은 건너뛴다");

    store.move_items(&[g3], DropTarget { section: None, before: Some(s), after: None });
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(scratch.data_file()).unwrap()).unwrap();
    assert_eq!(saved["sidebarOrder"], serde_json::json!([format!("group:{g3}"), format!("section:{s}")]), "원본과 같은 글자 형식");
    assert_eq!(saved["groups"][1]["sectionID"], serde_json::json!(s.to_string()), "그룹의 섹션 키");
    assert!(saved["groups"][0].get("sectionID").is_none(), "섹션 밖 그룹에는 적지 않는다");
    assert_eq!(saved["sections"][0]["isCollapsed"], serde_json::json!(false));

    // 섹션을 모두 지우면 이전 형식으로 돌아간다
    store.delete_section(s);
    let saved = fs::read_to_string(scratch.data_file()).unwrap();
    assert!(!saved.contains("sections") && !saved.contains("sidebarOrder") && !saved.contains("sectionID"));
    assert_eq!(outline(&store), "다 가 나");
}

// MARK: - 내보내기 · 가져오기

#[test]
fn export_import() {
    use linkgroupz_core::transfer::{read_document, ImportMode, ImportSummary};
    let scratch = Scratch::new();
    let mut store = LinkStore::open(scratch.data_file());
    let a = store.add_group(LinkGroup::new("A").with_links(vec![link("https://a.com")])).id;
    store.add_group(LinkGroup::new("B"));
    store.add_section("S", Some(a), &[a]);
    let archived = store.add_group(LinkGroup::new("보관")).id;
    store.archive_groups(&ids([archived]), Timestamp::now());
    let exported = store.export_data();
    assert_eq!(exported, fs::read(scratch.data_file()).unwrap(), "내보내기는 data.json 과 같다");

    let document = read_document(&exported).unwrap();
    assert_eq!(ImportSummary::of(&document), ImportSummary { groups: 2, links: 1, sections: 1, archived: 1, icon_images: 0 });
    assert!(read_document(b"{\"hello\":1}").is_err(), "데이터 파일이 아니면 거절");
    assert!(read_document(br#"{"version":99,"groups":[]}"#).is_err(), "새 버전 형식은 거절");

    // 더하기: 같은 파일을 더해도 id 가 겹치지 않게 둘 다 남는다
    store.import_document(read_document(&exported).unwrap(), ImportMode::Merge);
    assert_eq!(outline(&store), "[S: A] B [S: A] B", "끝에 같은 모양으로 더해진다");
    let group_ids: HashSet<Id> = store.groups().iter().map(|group| group.id).collect();
    assert_eq!(group_ids.len(), 4, "그룹 id 가 겹치지 않는다");
    let link_ids: HashSet<Id> = store.groups().iter().flat_map(|group| group.links.iter().map(|link| link.id)).collect();
    assert_eq!(link_ids.len(), 2, "페이지 id 도 겹치지 않는다");
    assert_eq!(store.sections().len(), 2);
    assert_eq!(store.archive().groups.len(), 2);

    // 바꾸기: 파일 내용 그대로
    store.import_document(read_document(&exported).unwrap(), ImportMode::Replace);
    assert_eq!(outline(&store), "[S: A] B");
    assert_eq!(store.export_data(), exported, "바꾼 뒤에는 가져온 파일과 같다");

    // 섹션이 없던 목록에 섹션 있는 파일을 더해도 기존 순서가 지켜진다
    let other = Scratch::new();
    let mut plain = LinkStore::open(other.data_file());
    plain.add_group(LinkGroup::new("X"));
    plain.add_group(LinkGroup::new("Y"));
    plain.import_document(read_document(&exported).unwrap(), ImportMode::Merge);
    assert_eq!(outline(&plain), "X Y [S: A] B");
}

/// 페이지 스위치(그룹을 열 때 열지)는 원본 LinkGroups 1.5.8 과 같은 이름(opensWithGroup)으로 읽고 쓴다.
#[test]
fn page_switch_matches_original() {
    let on: SavedLink = serde_json::from_str(r#"{"id":"00000000-0000-4000-8000-000000000001","url":"https://a.com"}"#).unwrap();
    assert!(on.opens_with_group, "이전 형식(칸 없음)은 켜짐");
    assert!(!serde_json::to_string(&on).unwrap().contains("opensWithGroup"), "켜져 있으면 적지 않아 이전 형식과 같다");

    let off: SavedLink = serde_json::from_str(r#"{"id":"00000000-0000-4000-8000-000000000002","url":"https://b.com","opensWithGroup":false}"#).unwrap();
    assert!(!off.opens_with_group);
    assert!(serde_json::to_string(&off).unwrap().contains(r#""opensWithGroup":false"#), "끈 것은 저장해도 남는다");

    let mut group = LinkGroup::new("g");
    group.links = vec![on.clone(), off.clone()];
    assert_eq!(group.for_opening().links, std::slice::from_ref(&on), "그룹을 열 때는 켠 페이지만");

    // 저장소에서 켜고 끄기 · 편집해도 스위치가 그대로
    let scratch = Scratch::new();
    let mut store = LinkStore::open(scratch.data_file());
    let id = store.add_group(group).id;
    store.set_link_opens_with_group(id, on.id, false);
    assert!(store.group(id).unwrap().for_opening().links.is_empty());
    store.set_link_opens_with_group(id, off.id, true);
    assert_eq!(store.group(id).unwrap().for_opening().links.len(), 1);
}

/// 보관함 1.5.7: 그 자리에 복원 · 보관 그룹 안 페이지 복원 · 삭제 · 보관 페이지를 보관 그룹에 넣기 · 순서 · 섹션 보관
#[test]
fn archive_drag_and_drop() {
    let scratch = Scratch::new();
    let mut store = LinkStore::open(scratch.data_file());
    let make = |store: &mut LinkStore, name: &str, urls: &[&str]| {
        let mut group = LinkGroup::new(name);
        group.links = urls.iter().map(|url| link(url)).collect();
        store.add_group(group).id
    };
    let a = make(&mut store, "A", &["https://a1.com", "https://a2.com"]);
    let b = make(&mut store, "B", &["https://b1.com"]);
    let c = make(&mut store, "C", &[]);
    let section = store.add_section("S", Some(b), &[b, c]).id;
    let at = Timestamp::now();

    // 그 자리에 복원: 보관한 A 를 섹션 S 의 C 앞으로
    store.archive_groups(&HashSet::from([a]), at);
    assert!(store.group(a).is_none());
    store.restore_groups_to(&[a], DropTarget { section: Some(section), before: Some(c), after: None });
    let in_section: Vec<Id> = store.groups().iter().filter(|group| group.section_id == Some(section)).map(|group| group.id).collect();
    assert_eq!(in_section, [b, a, c], "놓은 자리(섹션 안, C 앞)로 돌아온다");

    // 보관 그룹 안 페이지: 복원 · 삭제
    store.archive_groups(&HashSet::from([a]), at);
    let a1 = store.archive().groups[0].group.links[0].id;
    let a2 = store.archive().groups[0].group.links[1].id;
    let result = store.restore_links_from_archived_group(a, &HashSet::from([a1]), b);
    assert_eq!((result.moved, result.duplicates), (1, 0));
    assert!(store.group(b).unwrap().links.iter().any(|item| item.id == a1));
    store.delete_links_in_archived_group(a, &HashSet::from([a2]));
    assert!(store.archive().groups[0].group.links.is_empty());

    // 보관 페이지를 보관 그룹 안으로
    store.archive_links(&HashSet::from([a1]), b, at);
    store.move_archived_links_into_group(&HashSet::from([a1]), a);
    assert!(store.archive().links.is_empty());
    assert_eq!(store.archive().groups[0].group.links.len(), 1);

    // 보관 그룹 순서 · 보관 그룹 안 페이지 순서
    store.archive_groups(&HashSet::from([c]), at);
    assert_eq!(store.archive().groups.iter().map(|item| item.id()).collect::<Vec<_>>(), [c, a], "최근 보관이 위");
    store.move_archived_groups(&[0], 2);
    assert_eq!(store.archive().groups.iter().map(|item| item.id()).collect::<Vec<_>>(), [a, c]);

    // 섹션 보관: 안의 그룹을 보관하고 섹션은 지운다
    let d = make(&mut store, "D", &[]);
    let e = make(&mut store, "E", &[]);
    let other = store.add_section("T", Some(d), &[d, e]).id;
    store.archive_section(other, at);
    assert!(store.sections().iter().all(|item| item.id != other), "섹션은 지운다");
    assert!(store.group(d).is_none() && store.group(e).is_none());
    assert!(store.archive().groups.iter().any(|item| item.id() == d) && store.archive().groups.iter().any(|item| item.id() == e));
}

#[test]
fn replace_document_for_undo() {
    let scratch = Scratch::new();
    let mut store = LinkStore::open(scratch.data_file());
    let a = store.add_group(LinkGroup::new("A")).id;
    let section = store.add_section("S", Some(a), &[a]).id;
    let before = store.document();
    store.delete_groups(&HashSet::from([a]));
    store.set_section_collapsed(section, true);

    store.replace_document(before.clone());
    assert_eq!(store.groups().iter().map(|group| group.id).collect::<Vec<_>>(), [a], "지운 그룹이 돌아온다");
    assert!(store.sections()[0].is_collapsed, "섹션을 접은 상태는 지금 것을 그대로 둔다");
    assert_eq!(LinkStore::open(scratch.data_file()).groups().len(), 1, "파일에도 저장한다");
}
