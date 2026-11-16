//! 미리보기 창들이 함께 쓰는 브라우저 저장소 (macOS).
//!
//! 저장소는 디스크에 아무것도 남기지 않는 nonPersistent 이고, 로그인 쿠키는 키체인에만 보관한다 (`CookieVault`).
//! WebKit 객체는 메인 스레드에서만 만질 수 있어, 다른 스레드에서 부르면 메인 스레드로 넘겨 실행하고 결과를 기다린다.

use block2::RcBlock;
use linkgroupz_core::{StoredCookie, Timestamp};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::MainThreadMarker;
use objc2_foundation::{
    NSArray, NSDate, NSHTTPCookie, NSHTTPCookieDomain, NSHTTPCookieExpires, NSHTTPCookieName, NSHTTPCookiePath,
    NSHTTPCookieSameSitePolicy, NSHTTPCookieSecure, NSHTTPCookieValue, NSMutableDictionary, NSString,
};
use objc2_web_kit::{WKWebViewConfiguration, WKWebsiteDataStore};
use std::ptr::NonNull;
use std::sync::mpsc::{channel, Sender};
use std::time::Duration;
use tauri::AppHandle;

thread_local! {
    /// 앱이 켜져 있는 동안 하나만 쓰는 저장소. 메인 스레드에서만 만들어지고 쓰인다.
    static DATA_STORE: Retained<WKWebsiteDataStore> = {
        let mtm = MainThreadMarker::new().expect("미리보기 저장소는 메인 스레드에서만 쓴다");
        // Safety: 메인 스레드에서 부른다.
        unsafe { WKWebsiteDataStore::nonPersistentDataStore(mtm) }
    };
}

/// WebKit 이 응답하기를 기다리는 시간
const TIMEOUT: Duration = Duration::from_secs(10);

/// 공유 저장소를 쓰는 새 웹뷰 설정. 메인 스레드에서 부른다.
///
/// wry 가 설정에 스크립트 · 주소 처리기를 더하므로 웹뷰마다 새 설정을 만든다 (저장소만 함께 쓴다).
pub fn configuration(mtm: MainThreadMarker) -> Retained<WKWebViewConfiguration> {
    // Safety: 메인 스레드에서 부른다 (mtm 이 그 증거).
    let configuration = unsafe { WKWebViewConfiguration::new(mtm) };
    DATA_STORE.with(|store| unsafe { configuration.setWebsiteDataStore(store) });
    configuration
}

/// 메인 스레드에서 work 를 실행하고, work 가 보내 주는 값을 기다린다. 메인 스레드에서 부르면 안 된다.
fn on_main_thread<T: Send + 'static>(app: &AppHandle, work: impl FnOnce(Sender<T>) + Send + 'static) -> Option<T> {
    let (sender, receiver) = channel();
    app.run_on_main_thread(move || work(sender)).ok()?;
    receiver.recv_timeout(TIMEOUT).ok()
}

fn stored(cookie: &NSHTTPCookie) -> StoredCookie {
    StoredCookie {
        name: cookie.name().to_string(),
        value: cookie.value().to_string(),
        domain: cookie.domain().to_string(),
        path: cookie.path().to_string(),
        expires: cookie.expiresDate().map(|date| Timestamp::from_unix_seconds(date.timeIntervalSince1970())),
        secure: cookie.isSecure(),
        http_only: cookie.isHTTPOnly(),
        same_site: cookie.sameSitePolicy().map(|policy| policy.to_string()),
    }
}

fn native(cookie: &StoredCookie) -> Option<Retained<NSHTTPCookie>> {
    let properties = NSMutableDictionary::<NSString, AnyObject>::new();
    let text = |key: &'static NSString, value: &str| properties.insert(key, &*NSString::from_str(value));
    // Safety: 키들은 Foundation 이 내보내는 상수 문자열이고, 값은 그 키가 받는 형식(NSString · NSDate)이다.
    unsafe {
        text(NSHTTPCookieName, &cookie.name);
        text(NSHTTPCookieValue, &cookie.value);
        text(NSHTTPCookieDomain, &cookie.domain);
        text(NSHTTPCookiePath, &cookie.path);
        if let Some(expires) = cookie.expires {
            properties.insert(NSHTTPCookieExpires, &*NSDate::dateWithTimeIntervalSince1970(expires.unix_seconds()));
        }
        if cookie.secure {
            text(NSHTTPCookieSecure, "TRUE");
        }
        if cookie.http_only {
            properties.insert(&*NSString::from_str("HttpOnly"), &*NSString::from_str("TRUE"));
        }
        if let Some(same_site) = &cookie.same_site {
            text(NSHTTPCookieSameSitePolicy, same_site);
        }
        NSHTTPCookie::cookieWithProperties(&properties)
    }
}

/// 저장소의 모든 쿠키
pub fn all_cookies(app: &AppHandle) -> Vec<StoredCookie> {
    on_main_thread(app, |sender: Sender<Vec<StoredCookie>>| {
        DATA_STORE.with(|store| {
            let handler = RcBlock::new(move |cookies: NonNull<NSArray<NSHTTPCookie>>| {
                // Safety: WebKit 이 넘겨 준, 이 블록이 도는 동안 살아 있는 배열이다.
                let cookies = unsafe { cookies.as_ref() };
                let _ = sender.send(cookies.to_vec().iter().map(|cookie| stored(cookie)).collect());
            });
            // Safety: 메인 스레드에서 부른다.
            unsafe { store.httpCookieStore().getAllCookies(&handler) };
        });
    })
    .unwrap_or_default()
}

/// 쿠키들을 저장소에 넣고, 다 들어갈 때까지 기다린다.
pub fn set_cookies(app: &AppHandle, cookies: Vec<StoredCookie>) {
    if cookies.is_empty() {
        return;
    }
    on_main_thread(app, move |sender: Sender<()>| {
        DATA_STORE.with(|store| {
            // Safety: 메인 스레드에서 부른다.
            let cookie_store = unsafe { store.httpCookieStore() };
            for cookie in cookies.iter().filter_map(native) {
                unsafe { cookie_store.setCookie_completionHandler(&cookie, None) };
            }
            // 쿠키 저장소는 요청을 순서대로 처리한다. 뒤에 넣은 조회가 끝나면 앞의 쿠키들도 다 들어간 것이다.
            let handler = RcBlock::new(move |_: NonNull<NSArray<NSHTTPCookie>>| {
                let _ = sender.send(());
            });
            unsafe { cookie_store.getAllCookies(&handler) };
        });
    });
}

/// 저장소의 쿠키 · 캐시 등을 모두 지운다.
pub fn clear(app: &AppHandle) {
    on_main_thread(app, |sender: Sender<()>| {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        DATA_STORE.with(|store| {
            let handler = RcBlock::new(move || {
                let _ = sender.send(());
            });
            // Safety: 메인 스레드에서 부른다.
            unsafe {
                store.removeDataOfTypes_modifiedSince_completionHandler(
                    &WKWebsiteDataStore::allWebsiteDataTypes(mtm),
                    &NSDate::distantPast(),
                    &handler,
                );
            }
        });
    });
}
