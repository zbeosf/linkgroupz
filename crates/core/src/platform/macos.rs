use crate::browser::Browser;
use crate::secret::SecretError;
use crate::tabs::{ScriptFailure, ScriptValue};
use crate::timestamp::Timestamp;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::AnyThread;
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSRunningApplication, NSWorkspace};
use objc2_foundation::{
    NSAppleEventDescriptor, NSAppleScript, NSAppleScriptErrorMessage, NSAppleScriptErrorNumber, NSArray, NSDictionary,
    NSNumber, NSPoint, NSRect, NSSize, NSString,
};
use security_framework::passwords::{
    delete_generic_password, generic_password, set_generic_password_options, PasswordOptions,
};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// MARK: - 브라우저

pub fn browser_user_data_directory(browser: Browser) -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("/"));
    match browser {
        Browser::Chrome => base.join("Google/Chrome"),
        Browser::Edge => base.join("Microsoft Edge"),
        Browser::Whale => base.join("Naver/Whale"),
    }
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    /// 관리 환경설정(구성 프로파일로 강제한 값)까지 포함해 읽는다. 돌려받은 값은 호출한 쪽이 놓아 준다(Copy 규칙).
    fn CFPreferencesCopyAppValue(key: *const NSString, application_id: *const NSString) -> *mut AnyObject;
}

/// 관리 정책 값. 회사가 구성 프로파일로 내려 준 브라우저 정책은 그 브라우저의 환경설정 영역에 강제 값으로 들어 있다.
pub fn browser_policy_value(browser: Browser, name: &str) -> Option<i64> {
    // 정책이 들어가는 영역 이름은 앱 식별자와 다를 수 있다 (Edge 는 com.microsoft.Edge).
    let domain = match browser {
        Browser::Chrome => "com.google.Chrome",
        Browser::Edge => "com.microsoft.Edge",
        Browser::Whale => "com.naver.Whale",
    };
    let (key, domain) = (NSString::from_str(name), NSString::from_str(domain));
    // Safety: 두 인자는 살아 있는 NSString(CFString 과 같다)이고, 돌려받은 객체는 +1 이라 Retained 가 놓아 준다.
    let value = unsafe { Retained::from_raw(CFPreferencesCopyAppValue(&*key, &*domain)) }?;
    let number = value.downcast_ref::<NSNumber>()?;
    Some(number.longLongValue())
}

pub fn browser_app_path(browser: Browser) -> Option<PathBuf> {
    let workspace = NSWorkspace::sharedWorkspace();
    let url = workspace.URLForApplicationWithBundleIdentifier(&NSString::from_str(browser.bundle_identifier()))?;
    Some(PathBuf::from(url.path()?.to_string()))
}

fn running_applications(browser: Browser) -> Vec<Retained<NSRunningApplication>> {
    let identifier = NSString::from_str(browser.bundle_identifier());
    NSRunningApplication::runningApplicationsWithBundleIdentifier(&identifier)
        .to_vec()
        .into_iter()
        .filter(|app| !app.isTerminated())
        .collect()
}

pub fn browser_is_running(browser: Browser) -> bool {
    !running_applications(browser).is_empty()
}

pub fn browser_launch_date(browser: Browser) -> Option<Timestamp> {
    let app = running_applications(browser).into_iter().next()?;
    Some(Timestamp::from_unix_seconds(app.launchDate()?.timeIntervalSince1970()))
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    /// 화면의 창 목록 (CFArray<CFDictionary>). 돌려받은 값은 호출한 쪽이 놓아 준다(Copy 규칙).
    fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> *mut NSArray<NSDictionary<NSString, AnyObject>>;
}

/// 브라우저의 지금 보이는 일반 창 번호들. 창 이름은 읽지 않으므로 화면 기록 권한이 필요 없다.
pub fn browser_window_numbers(browser: Browser) -> std::collections::HashSet<u32> {
    const ON_SCREEN_ONLY: u32 = 1 << 0;
    const EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;
    let pids: std::collections::HashSet<i32> = running_applications(browser).iter().map(|app| app.processIdentifier()).collect();
    if pids.is_empty() {
        return Default::default();
    }
    // Safety: 인자는 정수뿐이고, 돌려받은 배열은 +1 이라 Retained 가 놓아 준다.
    let Some(windows) = (unsafe { Retained::from_raw(CGWindowListCopyWindowInfo(ON_SCREEN_ONLY | EXCLUDE_DESKTOP_ELEMENTS, 0)) }) else {
        return Default::default();
    };
    let number = |window: &NSDictionary<NSString, AnyObject>, key: &str| {
        window.objectForKey(&NSString::from_str(key)).and_then(|value| value.downcast::<NSNumber>().ok()).map(|value| value.longLongValue())
    };
    windows
        .to_vec()
        .iter()
        .filter(|window| number(window, "kCGWindowLayer") == Some(0))
        .filter(|window| number(window, "kCGWindowOwnerPID").is_some_and(|pid| pids.contains(&(pid as i32))))
        .filter_map(|window| number(window, "kCGWindowNumber").map(|number| number as u32))
        .collect()
}

/// 지금 맨 앞에 있는 앱의 번들 식별자
pub fn frontmost_bundle_identifier() -> Option<String> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    Some(app.bundleIdentifier()?.to_string())
}

/// 앱 번들의 아이콘을 PNG 로 만든다.
pub fn app_icon_png(app: &Path, size: u32) -> Option<Vec<u8>> {
    let workspace = NSWorkspace::sharedWorkspace();
    let icon = workspace.iconForFile(&NSString::from_str(app.to_str()?));
    // 화면 배율 2배까지 선명하도록 요청한 크기의 2배에 가장 가까운 그림을 고르게 한다.
    let side = f64::from(size) * 2.0;
    let mut rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(side, side));
    // Safety: rect 는 살아 있는 지역 변수이고, context · hints 는 넘기지 않는다.
    let cg_image = unsafe { icon.CGImageForProposedRect_context_hints(&mut rect, None, None) }?;
    let rep = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &cg_image);
    let properties = NSDictionary::<NSString, AnyObject>::new();
    // Safety: 빈 속성 사전을 넘긴다.
    let png = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties) }?;
    let decoded = image::load_from_memory(&png.to_vec()).ok()?.into_rgba8();
    super::icon_to_png(decoded, size)
}

// MARK: - 실행

fn spawn_detached(command: &mut Command) -> io::Result<()> {
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(crate::tr!("open 이 실패했습니다 ({})", "open failed ({})", status)))
    }
}

/// 주소를 그 앱으로 연다 (이미 실행 중이면 그 앱에 넘긴다).
pub fn open_urls(app: &Path, urls: &[String]) -> io::Result<()> {
    spawn_detached(Command::new("/usr/bin/open").arg("-a").arg(app).args(urls))
}

/// 실행 인자를 붙여 앱을 띄운다. 실행 중인 브라우저에도 인자가 전달된다.
pub fn launch_with_arguments(app: &Path, arguments: &[String]) -> io::Result<()> {
    spawn_detached(Command::new("/usr/bin/open").arg("-n").arg("-a").arg(app).arg("--args").args(arguments))
}

// MARK: - AppleScript

/// AppleScript 를 실행해 결과를 값으로 돌려준다. 메인 스레드에서 불러야 한다 (NSAppleScript 는 스레드에 안전하지 않다).
pub fn run_apple_script(source: &str) -> Result<ScriptValue, ScriptFailure> {
    let script = NSAppleScript::initWithSource(NSAppleScript::alloc(), &NSString::from_str(source))
        .ok_or_else(|| ScriptFailure { number: 0, message: Some(crate::tr!("스크립트를 만들지 못했습니다", "Couldn't create the script").to_owned()) })?;
    let mut error_info: Option<Retained<NSDictionary<NSString, AnyObject>>> = None;
    // objc2-foundation 의 executeAndReturnError 는 nil 을 돌려줄 수 없다고 선언돼 있지만, 스크립트가 실패하면 nil 이 온다.
    // 그래서 돌려주는 값을 Option 으로 받도록 직접 보낸다.
    // Safety: 선택자와 인자 형식은 -[NSAppleScript executeAndReturnError:] 와 같고,
    // error_info 는 NSAppleScript 가 채워 주는 [NSString: 임의 객체] 사전이다.
    let result: Option<Retained<NSAppleEventDescriptor>> =
        unsafe { objc2::msg_send![&*script, executeAndReturnError: Some(&mut error_info)] };
    if let Some(info) = error_info {
        // Safety: 두 키는 Foundation 이 내보내는 상수 문자열이다.
        let (number_key, message_key) = unsafe { (NSAppleScriptErrorNumber, NSAppleScriptErrorMessage) };
        let number = info
            .objectForKey(number_key)
            .and_then(|value| value.downcast::<NSNumber>().ok())
            .map(|value| value.as_i64())
            .unwrap_or(0);
        let message = info
            .objectForKey(message_key)
            .and_then(|value| value.downcast::<NSString>().ok())
            .map(|value| value.to_string());
        return Err(ScriptFailure { number, message });
    }
    Ok(result.map_or(ScriptValue::Null, |descriptor| script_value(&descriptor, 0)))
}

const fn four_char_code(code: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*code)
}

fn script_value(descriptor: &NSAppleEventDescriptor, depth: usize) -> ScriptValue {
    const TYPE_LIST: u32 = four_char_code(b"list");
    const TYPE_TRUE: u32 = four_char_code(b"true");
    const TYPE_FALSE: u32 = four_char_code(b"fals");
    const TYPE_BOOLEAN: u32 = four_char_code(b"bool");
    const TYPE_INT32: u32 = four_char_code(b"long");
    const TYPE_INT16: u32 = four_char_code(b"shor");
    const TYPE_NULL: u32 = four_char_code(b"null");

    // descriptorType 은 objc2-core-services 기능 뒤에 있어 직접 보낸다. DescType 은 FourCharCode(u32)다.
    // Safety: descriptorType 은 인자 없이 u32 를 돌려주는 메서드다.
    let kind: u32 = unsafe { objc2::msg_send![descriptor, descriptorType] };
    match kind {
        TYPE_LIST if depth < 8 => {
            let count = descriptor.numberOfItems();
            let items = (1..=count)
                .map(|index| match descriptor.descriptorAtIndex(index) {
                    Some(item) => script_value(&item, depth + 1),
                    None => ScriptValue::Null,
                })
                .collect();
            ScriptValue::List(items)
        }
        TYPE_TRUE => ScriptValue::Bool(true),
        TYPE_FALSE => ScriptValue::Bool(false),
        TYPE_BOOLEAN => ScriptValue::Bool(descriptor.int32Value() != 0),
        TYPE_INT32 | TYPE_INT16 => ScriptValue::Int(i64::from(descriptor.int32Value())),
        TYPE_NULL => ScriptValue::Null,
        _ => match descriptor.stringValue() {
            Some(text) => ScriptValue::Text(text.to_string()),
            None => ScriptValue::Null,
        },
    }
}

// MARK: - 비밀 값 (로그인 키체인의 일반 암호 항목)

const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

fn secret_error(error: security_framework::base::Error) -> SecretError {
    SecretError::Store(error.message().unwrap_or_else(|| error.code().to_string()))
}

pub fn secret_load(service: &str, account: &str) -> Result<Option<Vec<u8>>, SecretError> {
    match generic_password(PasswordOptions::new_generic_password(service, account)) {
        Ok(data) => Ok(Some(data)),
        Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(None),
        Err(error) => Err(secret_error(error)),
    }
}

pub fn secret_save(service: &str, account: &str, label: &str, data: &[u8]) -> Result<(), SecretError> {
    let options = || {
        let mut options = PasswordOptions::new_generic_password(service, account);
        options.set_label(label);
        options
    };
    match set_generic_password_options(data, options()) {
        Ok(()) => Ok(()),
        // 이미 있는 항목의 이름표가 달라 고치지 못한 경우: 지우고 다시 넣는다.
        Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => {
            secret_delete(service, account)?;
            set_generic_password_options(data, options()).map_err(secret_error)
        }
        Err(error) => Err(secret_error(error)),
    }
}

pub fn secret_delete(service: &str, account: &str) -> Result<(), SecretError> {
    match delete_generic_password(service, account) {
        Ok(()) => Ok(()),
        Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(()),
        Err(error) => Err(secret_error(error)),
    }
}
