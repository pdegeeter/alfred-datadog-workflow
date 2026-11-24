//! Percent-encoding of query-string values.

/// Percent-encode a query-string value: RFC 3986 unreserved characters are
/// kept, everything else (spaces, `:`, `#`, UTF-8 bytes) is `%XX`.
pub fn encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}
