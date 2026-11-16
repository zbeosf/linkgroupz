//! Chrome · Edge 가 LinkGroupz 확장의 sendNativeMessage 마다 띄우는 Native Messaging 호스트.
//!
//! 받은 탭 · 탭 그룹 스냅샷을 보낸 브라우저의 {browser}-snapshot.json 에 쓰고 결과를 돌려준 뒤 끝난다.
//! 등록 파일: `NativeSnapshotStore::manifest_path` (앱이 실행될 때 브라우저마다 씀)

use linkgroupz_core::{LinkStore, NativeMessaging, NativeSnapshotStore};
use std::io::{self, Write};

fn main() {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    // 앱 · 예시 데이터로 띄운 앱 모두 실제 앱 데이터 폴더의 파일을 읽는다 (NativeSnapshotStore::file_path).
    let directory = LinkStore::default_data_directory();
    while let Some(message) = NativeMessaging::read_message(&mut input) {
        let reply = match NativeSnapshotStore::write(&message, &directory) {
            Ok(_) => serde_json::json!({ "ok": true }),
            Err(error) => serde_json::json!({ "ok": false, "error": error.to_string() }),
        };
        let encoded = NativeMessaging::encode(reply.to_string().as_bytes());
        if output.write_all(&encoded).and_then(|()| output.flush()).is_err() {
            break;
        }
    }
}
