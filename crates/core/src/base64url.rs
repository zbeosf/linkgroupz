use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;

/// URL 조각(#...)에 실을 수 있는 base64: `+` `/` 대신 `-` `_` 를 쓰고 `=` 는 뗀다.
pub struct Base64Url;

impl Base64Url {
    pub fn encode(data: &[u8]) -> String {
        URL_SAFE_NO_PAD.encode(data)
    }

    pub fn decode(text: &str) -> Option<Vec<u8>> {
        let mut encoded = text.replace('-', "+").replace('_', "/");
        while !encoded.len().is_multiple_of(4) {
            encoded.push('=');
        }
        STANDARD.decode(encoded).ok()
    }
}
