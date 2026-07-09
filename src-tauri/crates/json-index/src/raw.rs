//! Low-level byte scanning helpers shared by the validating scanner (build
//! time) and the trusting entry walker (query time, input already validated).

use memchr::memchr2;

#[inline]
pub(crate) fn skip_ws(buf: &[u8], pos: &mut usize) {
    while *pos < buf.len() && matches!(buf[*pos], b' ' | b'\t' | b'\n' | b'\r') {
        *pos += 1;
    }
}

/// `pos` must point at the opening quote. Advances past the closing quote.
/// Returns false on unterminated string.
#[inline]
pub(crate) fn skip_string(buf: &[u8], pos: &mut usize) -> bool {
    debug_assert_eq!(buf[*pos], b'"');
    let mut p = *pos + 1;
    loop {
        match memchr2(b'"', b'\\', &buf[p..]) {
            None => return false,
            Some(i) => {
                if buf[p + i] == b'"' {
                    *pos = p + i + 1;
                    return true;
                }
                // Backslash: skip the escape character after it. For \uXXXX the
                // hex digits contain no '"' or '\', so continuing is safe.
                if p + i + 2 > buf.len() {
                    return false;
                }
                p = p + i + 2;
            }
        }
    }
}

/// `pos` points at '-' or a digit. Consumes the number's characters.
/// Structural validation only — accepts some malformed numbers by design
/// (raw text is preserved end-to-end, never parsed to float).
#[inline]
pub(crate) fn skip_number(buf: &[u8], pos: &mut usize) {
    while *pos < buf.len()
        && matches!(buf[*pos], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')
    {
        *pos += 1;
    }
}

/// Matches an exact literal (true/false/null). Returns false on mismatch.
#[inline]
pub(crate) fn skip_literal(buf: &[u8], pos: &mut usize, lit: &[u8]) -> bool {
    if buf.len() - *pos >= lit.len() && &buf[*pos..*pos + lit.len()] == lit {
        *pos += lit.len();
        true
    } else {
        false
    }
}

/// Unescape the contents of a JSON string (bytes between the quotes).
/// Invalid escapes degrade to U+FFFD instead of failing — display helper only.
pub fn unescape(raw: &[u8]) -> String {
    if !raw.contains(&b'\\') {
        return String::from_utf8_lossy(raw).into_owned();
    }
    let mut out: Vec<u8> = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] != b'\\' {
            out.push(raw[i]);
            i += 1;
            continue;
        }
        i += 1;
        if i >= raw.len() {
            out.extend_from_slice("\u{FFFD}".as_bytes());
            break;
        }
        match raw[i] {
            b'"' => out.push(b'"'),
            b'\\' => out.push(b'\\'),
            b'/' => out.push(b'/'),
            b'b' => out.push(0x08),
            b'f' => out.push(0x0C),
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b't' => out.push(b'\t'),
            b'u' => {
                let (cp, consumed) = parse_unicode_escape(&raw[i..]);
                out.extend_from_slice(cp.encode_utf8(&mut [0u8; 4]).as_bytes());
                i += consumed - 1;
            }
            _ => out.extend_from_slice("\u{FFFD}".as_bytes()),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `raw` starts at the 'u' of a \uXXXX escape. Returns (char, bytes consumed
/// starting from and including the 'u'). Handles surrogate pairs.
fn parse_unicode_escape(raw: &[u8]) -> (char, usize) {
    fn hex4(b: &[u8]) -> Option<u32> {
        if b.len() < 4 {
            return None;
        }
        let s = std::str::from_utf8(&b[..4]).ok()?;
        u32::from_str_radix(s, 16).ok()
    }
    let Some(hi) = hex4(&raw[1..]) else {
        return ('\u{FFFD}', 1);
    };
    if (0xD800..=0xDBFF).contains(&hi) {
        // Expect a low surrogate: \uXXXX
        if raw.len() >= 11 && raw[5] == b'\\' && raw[6] == b'u' {
            if let Some(lo) = hex4(&raw[7..]) {
                if (0xDC00..=0xDFFF).contains(&lo) {
                    let cp = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                    return (char::from_u32(cp).unwrap_or('\u{FFFD}'), 11);
                }
            }
        }
        return ('\u{FFFD}', 5);
    }
    (char::from_u32(hi).unwrap_or('\u{FFFD}'), 5)
}
