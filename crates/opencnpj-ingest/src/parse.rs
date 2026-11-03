//! Parser for the RFB CSV dialect.
//!
//! Every field is wrapped in double quotes and separated by `;`. Quotes inside values are
//! not escaped, so a record is split on the `";"` sequence rather than with a generic CSV
//! state machine. A record with too few fields is joined with the following lines
//! (embedded line breaks); any line that still does not fit the layout is returned as a
//! rejected line, verbatim, so it can be stored and reported. Nothing is silently dropped.

use memchr::memchr_iter;

/// Maximum physical lines merged into one record.
const MAX_JOIN: usize = 16;

#[derive(Debug, Clone)]
pub struct RejectedLine {
    pub line: u64,
    pub reason: &'static str,
    pub raw: Vec<u8>,
}

/// Splits one logical record into fields (borrowed from `rec`).
pub fn split_fields<'a>(rec: &'a [u8], out: &mut Vec<&'a [u8]>) {
    out.clear();
    if rec.len() >= 2 && rec[0] == b'"' && rec[rec.len() - 1] == b'"' {
        let inner = &rec[1..rec.len() - 1];
        let mut start = 0;
        let mut i = 0;
        while let Some(p) = memchr::memchr(b'"', &inner[i..]) {
            let q = i + p;
            if inner.get(q + 1) == Some(&b';') && inner.get(q + 2) == Some(&b'"') {
                out.push(&inner[start..q]);
                start = q + 3;
                i = start;
            } else {
                i = q + 1;
            }
        }
        out.push(&inner[start..]);
    } else {
        out.extend(rec.split(|&b| b == b';'));
    }
}

/// Iterates records of `buf` (complete lines). `first_line` is the 1-based number of the
/// first line in `buf`.
pub fn for_each_record<'a>(
    buf: &'a [u8],
    expected: usize,
    first_line: u64,
    mut f: impl FnMut(Result<(u64, &'a [u8], &[&'a [u8]]), RejectedLine>),
) {
    let mut ends: Vec<usize> = memchr_iter(b'\n', buf).collect();
    if buf.last().is_some_and(|&b| b != b'\n') {
        ends.push(buf.len());
    }
    let mut fields: Vec<&'a [u8]> = Vec::with_capacity(expected + 2);
    let mut i = 0;
    let mut start = 0;
    while i < ends.len() {
        let line_no = first_line + i as u64;
        let mut joined = 0;
        let mut ok = false;
        let mut rec: &'a [u8];
        loop {
            rec = trim_cr(&buf[start..ends[i + joined]]);
            split_fields(rec, &mut fields);
            if fields.len() == expected {
                ok = true;
                break;
            }
            if fields.len() > expected || joined + 1 >= MAX_JOIN || i + joined + 1 >= ends.len() {
                break;
            }
            // Never swallow a line that looks like the start of the next record.
            if looks_like_record_start(&buf[ends[i + joined] + 1..ends[i + joined + 1]]) {
                break;
            }
            joined += 1;
        }
        if ok {
            f(Ok((line_no, rec, &fields)));
            start = ends[i + joined] + 1;
            i += joined + 1;
        } else {
            let raw = trim_cr(&buf[start..ends[i]]);
            let reason = if raw.is_empty() { "empty line" } else { "field count does not match layout" };
            f(Err(RejectedLine { line: line_no, reason, raw: raw.to_vec() }));
            start = ends[i] + 1;
            i += 1;
        }
    }
}

/// `"XXXXXXXX";"` — a quoted 8-character key followed by a delimiter.
fn looks_like_record_start(line: &[u8]) -> bool {
    line.len() >= 12 && line[0] == b'"' && line[1..9].iter().all(u8::is_ascii_alphanumeric) && &line[9..12] == b"\";\""
}

/// Removes a CR only when it terminates a quoted record (CRLF files).
fn trim_cr(rec: &[u8]) -> &[u8] {
    match rec {
        [rest @ .., b'"', b'\r'] => &rec[..rest.len() + 1],
        _ => rec,
    }
}

/// Finds a cut point near `target` where a new record starts (`\n` followed by `"`), so
/// chunks never split a record. Returns the index just after the newline.
pub fn chunk_boundary(buf: &[u8], target: usize) -> Option<usize> {
    let mut i = target.min(buf.len());
    while let Some(p) = memchr::memchr(b'\n', &buf[i..]) {
        let nl = i + p;
        match buf.get(nl + 1) {
            Some(b'"') => return Some(nl + 1),
            None => return None,
            _ => i = nl + 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(buf: &[u8], n: usize) -> (Vec<(u64, Vec<String>)>, Vec<RejectedLine>) {
        let mut ok = Vec::new();
        let mut bad = Vec::new();
        for_each_record(buf, n, 1, |r| match r {
            Ok((l, _, f)) => ok.push((l, f.iter().map(|x| String::from_utf8_lossy(x).into_owned()).collect())),
            Err(e) => bad.push(e),
        });
        (ok, bad)
    }

    #[test]
    fn plain_records() {
        let (ok, bad) = collect(b"\"00000000\";\"BANCO DO BRASIL SA\";\"\"\n\"1\";\"A\";\"B\"\n", 3);
        assert!(bad.is_empty());
        assert_eq!(ok[0].1, ["00000000", "BANCO DO BRASIL SA", ""]);
        assert_eq!(ok[1], (2, vec!["1".into(), "A".into(), "B".into()]));
    }

    #[test]
    fn unescaped_quotes_inside_values() {
        let (ok, _) = collect(b"\"1\";\"BAR \"DO ZE\" LTDA\";\"X\"\n\"2\";\"ABC\"\";\"Y\"", 3);
        assert_eq!(ok[0].1[1], "BAR \"DO ZE\" LTDA");
        assert_eq!(ok[1].1[1], "ABC\"");
    }

    #[test]
    fn embedded_newline_is_joined_and_bad_lines_are_kept() {
        let (ok, bad) = collect(b"\"1\";\"LINE\nBREAK\";\"X\"\n\"2\";\"short\"\n\"3\";\"a\";\"b\"\r\n", 3);
        assert_eq!(ok[0], (1, vec!["1".into(), "LINE\nBREAK".into(), "X".into()]));
        assert_eq!(bad.len(), 1);
        assert_eq!(bad[0].line, 3);
        assert_eq!(bad[0].raw, b"\"2\";\"short\"");
        assert_eq!(ok[1], (4, vec!["3".into(), "a".into(), "b".into()]));
    }

    #[test]
    fn single_field_garbage_does_not_swallow_next_record() {
        let (ok, bad) = collect(
            b"garbage
\"12345678\";\"a\";\"b\"\n",
            3,
        );
        assert_eq!(bad.len(), 1);
        assert_eq!(ok, vec![(2, vec!["12345678".into(), "a".into(), "b".into()])]);
    }

    #[test]
    fn boundaries() {
        let buf = b"\"a\"\n\"b\nc\"\n\"d\"\n";
        assert_eq!(chunk_boundary(buf, 0), Some(4));
        assert_eq!(chunk_boundary(buf, 5), Some(10));
        assert_eq!(chunk_boundary(buf, 11), None);
    }
}
