//! Line-based parser for the systemd unit/INI format.
//!
//! This follows the semantics of systemd's own `config_parse()` (`src/shared/conf-parser.c`):
//!
//! - Lines are stripped of surrounding whitespace.
//! - Lines starting with `#` or `;` (after leading whitespace) are comments, even in the middle of
//!   a continued value.
//! - A line ending with an unescaped `\` is continued on the next line; the backslash is replaced
//!   with a space and the next line is appended verbatim.
//! - Values are not unquoted or unescaped, as in systemd this is left to the individual settings.
//! - Sections with the same name are merged, and repeated keys are kept in order.
//!
//! See also <https://www.freedesktop.org/software/systemd/man/latest/systemd.syntax.html>.

use std::collections::HashMap;

pub(crate) type IniIntermediate = HashMap<String, Vec<(String, String)>>;
pub type Err = ParseError;

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error("line {line}: {kind}")]
pub struct ParseError {
    /// 1-indexed line number the error occurred on.
    pub line: usize,
    pub kind: ParseErrorKind,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    #[error("invalid section header {0:?}")]
    InvalidSectionHeader(String),
    #[error("assignment outside of section: {0:?}")]
    AssignmentOutsideSection(String),
    #[error("missing '=' in {0:?}")]
    MissingEquals(String),
    #[error("missing key name before '=' in {0:?}")]
    MissingKey(String),
}

const COMMENTS: [char; 2] = ['#', ';'];
const UTF8_BOM: char = '\u{feff}';

/// Whether `line` ends in a backslash that is not itself escaped.
fn ends_with_continuation(line: &str) -> bool {
    let mut escaped = false;
    for c in line.chars() {
        escaped = !escaped && c == '\\';
    }
    escaped
}

fn parse_line(
    ini: &mut IniIntermediate,
    section: &mut Option<String>,
    line_no: usize,
    line: &str,
) -> Result<(), ParseError> {
    let err = |kind| ParseError {
        line: line_no,
        kind,
    };
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }

    if let Some(header) = line.strip_prefix('[') {
        let name = header
            .strip_suffix(']')
            .ok_or_else(|| err(ParseErrorKind::InvalidSectionHeader(line.to_owned())))?;
        ini.entry(name.to_owned()).or_default();
        *section = Some(name.to_owned());
        return Ok(());
    }

    let Some(section) = section else {
        return Err(err(ParseErrorKind::AssignmentOutsideSection(
            line.to_owned(),
        )));
    };
    let (key, value) = line
        .split_once('=')
        .ok_or_else(|| err(ParseErrorKind::MissingEquals(line.to_owned())))?;
    let key = key.trim();
    if key.is_empty() {
        return Err(err(ParseErrorKind::MissingKey(line.to_owned())));
    }
    ini.entry(section.clone())
        .or_default()
        .push((key.to_owned(), value.trim().to_owned()));
    Ok(())
}

/// Parse a string into an intermediate representation of an INI file.
///
/// # Errors
///
/// Returns an error if the input is not a valid systemd unit file.
pub fn parse_str(s: &str) -> Result<IniIntermediate, Err> {
    let s = s.strip_prefix(UTF8_BOM).unwrap_or(s);
    let mut ini = IniIntermediate::new();
    let mut section = None;
    let mut continuation: Option<String> = None;
    let mut line_no = 0;

    for line in s.lines() {
        line_no += 1;
        if line.trim_start().starts_with(COMMENTS) {
            continue;
        }

        let mut joined = continuation.take().unwrap_or_default();
        joined.push_str(line);
        if ends_with_continuation(line) {
            joined.pop();
            joined.push(' ');
            continuation = Some(joined);
            continue;
        }
        parse_line(&mut ini, &mut section, line_no, &joined)?;
    }
    if let Some(joined) = continuation {
        parse_line(&mut ini, &mut section, line_no, &joined)?;
    }

    Ok(ini)
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn kv(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|&(k, v)| (k.to_owned(), v.to_owned()))
            .collect()
    }

    #[test]
    fn sanitytest_conf() {
        let ini = parse_str(include_str!("../test/sanitytest.conf")).unwrap();
        assert_eq!(
            ini["Section A"],
            kv(&[("KeyOne", "value 1"), ("KeyTwo", "value 2")])
        );
        assert_eq!(
            ini["Section B"],
            kv(&[
                ("Setting", r#""something" "some thing" "…""#),
                ("KeyTwo", "value 2         value 2 continued"),
            ])
        );
        assert_eq!(
            ini["Section C"],
            kv(&[("KeyThree", "value 3        value 3 continued")])
        );
    }

    #[test]
    fn quotes_are_preserved() {
        // systemd leaves unquoting to the individual settings
        let ini = parse_str("[section]\nTheFunny=\"this is str\"\nOther=\"a\" b\n").unwrap();
        assert_eq!(
            ini["section"],
            kv(&[("TheFunny", "\"this is str\""), ("Other", "\"a\" b")])
        );
    }

    #[test]
    fn whitespace_is_trimmed() {
        let ini = parse_str("  [A] \n\tk = v  \n").unwrap();
        assert_eq!(ini["A"], kv(&[("k", "v")]));
    }

    #[test]
    fn comments() {
        let ini =
            parse_str("# top\n; also top\n[A]\n  # indented\nk=v # not a comment\n# end").unwrap();
        assert_eq!(ini["A"], kv(&[("k", "v # not a comment")]));
    }

    #[test]
    fn continuation() {
        let ini = parse_str("[A]\nk=a\\\nb\nl=c\\\n# skipped\nd\nm=e\\\\\nn=f\\").unwrap();
        assert_eq!(
            ini["A"],
            kv(&[("k", "a b"), ("l", "c d"), ("m", "e\\\\"), ("n", "f")])
        );
    }

    #[test]
    fn duplicate_sections_are_merged() {
        let ini = parse_str("[A]\nx=1\n[B]\nz=3\n[A]\ny=2\nx=4\n").unwrap();
        assert_eq!(ini["A"], kv(&[("x", "1"), ("y", "2"), ("x", "4")]));
        assert_eq!(ini["B"], kv(&[("z", "3")]));
    }

    #[test]
    fn keys_with_punctuation() {
        let ini = parse_str("[X-Section]\nX-Foo=bar\nKey.With@Odd_chars=baz=qux\n").unwrap();
        assert_eq!(
            ini["X-Section"],
            kv(&[("X-Foo", "bar"), ("Key.With@Odd_chars", "baz=qux")])
        );
    }

    #[test]
    fn crlf_and_bom() {
        let ini = parse_str("\u{feff}[A]\r\nk=v\r\nempty=\r\n").unwrap();
        assert_eq!(ini["A"], kv(&[("k", "v"), ("empty", "")]));
    }

    #[test]
    fn empty_input_and_sections() {
        assert!(parse_str("").unwrap().is_empty());
        assert!(parse_str("# only a comment\n\n").unwrap().is_empty());
        assert!(parse_str("[A]\n").unwrap()["A"].is_empty());
    }

    #[test]
    fn errors() {
        let e = |s| parse_str(s).unwrap_err();
        assert_eq!(e("k=v\n[A]").line, 1);
        assert!(matches!(
            e("k=v").kind,
            ParseErrorKind::AssignmentOutsideSection(_)
        ));
        assert!(matches!(
            e("[A\n").kind,
            ParseErrorKind::InvalidSectionHeader(_)
        ));
        assert_eq!(
            e("[A]\n\nnoequals\n"),
            ParseError {
                line: 3,
                kind: ParseErrorKind::MissingEquals("noequals".to_owned())
            }
        );
        assert!(matches!(e("[A]\n=v").kind, ParseErrorKind::MissingKey(_)));
    }
}
