use crate::windows;
use linkgroupz_core::{ScriptFailure, ScriptValue, TabsHost};
use tauri::AppHandle;

/// 탭 읽기(백그라운드 스레드)가 부탁하는 일을 앱의 메인 스레드에서 해 준다.
pub struct AppTabsHost {
    app: AppHandle,
}

impl AppTabsHost {
    pub fn new(app: &AppHandle) -> Self {
        Self { app: app.clone() }
    }
}

impl TabsHost for AppTabsHost {
    #[cfg(target_os = "macos")]
    fn run_script(&self, source: &str) -> Result<ScriptValue, ScriptFailure> {
        let failure = |message: &str| ScriptFailure { number: 0, message: Some(message.to_owned()) };
        let (sender, receiver) = std::sync::mpsc::channel();
        let source = source.to_owned();
        // NSAppleScript 는 메인 스레드에서 실행해야 한다.
        self.app
            .run_on_main_thread(move || {
                let _ = sender.send(linkgroupz_core::platform::run_apple_script(&source));
            })
            .map_err(|_| failure(linkgroupz_core::tr!("스크립트를 실행하지 못했습니다", "Couldn't run the script")))?;
        receiver.recv().map_err(|_| failure(linkgroupz_core::tr!("스크립트 결과를 받지 못했습니다", "Didn't get the script result")))?
    }

    #[cfg(not(target_os = "macos"))]
    fn run_script(&self, _source: &str) -> Result<ScriptValue, ScriptFailure> {
        Err(ScriptFailure { number: 0, message: Some(linkgroupz_core::tr!("AppleScript 는 macOS 에서만 쓸 수 있습니다", "AppleScript is only available on macOS").to_owned()) })
    }

    fn reactivate_app(&self) {
        windows::focus_manager(&self.app);
    }
}
