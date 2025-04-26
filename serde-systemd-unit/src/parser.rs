use chumsky::prelude::*;
use std::collections::HashMap;

pub(crate) type IniIntermediate = HashMap<String, Vec<(String, String)>>;
pub(crate) type Err<'s> = Vec<chumsky::error::Rich<'s, char>>;
pub(crate) type Ext<'s> = chumsky::extra::Err<Rich<'s, char>>;

#[must_use]
fn parser<'s>() -> impl Parser<'s, &'s str, IniIntermediate, Ext<'s>> {
    let inline_space = any::<_, Ext>()
        .filter(|c: &char| c.is_whitespace() && *c != '\n')
        .repeated();
    let empty_lines = just('\n').then(inline_space).repeated().ignored();
    let root_comments = (just('#').or(just(';')))
        .then(none_of('\n').repeated())
        .separated_by(empty_lines);
    let newline_or_comment = root_comments
        .clone()
        .allow_leading()
        .allow_trailing()
        .ignored();
    let uni = |radix: u32, len: usize| {
        let p = any().filter(move |c: &char| c.is_digit(radix));
        p.repeated()
            .exactly(len)
            .collect()
            .validate(move |digits: String, e, emitter| {
                char::from_u32(u32::from_str_radix(&digits, radix).unwrap()).unwrap_or_else(|| {
                    emitter.emit(Rich::custom(e.span(), "invalid unicode character"));
                    '\u{FFFD}' // unicode replacement character
                })
            })
    };

    let esc = just('\\')
        .ignore_then(choice((
            just('a').to(Some(0x07 as char)),
            just('b').to(Some(0x08 as char)),
            just('f').to(Some(0x0c as char)),
            just('n').to(Some('\n')),
            just('r').to(Some('\r')),
            just('t').to(Some('\t')),
            just('v').to(Some(0x0b as char)),
            just('\\').to(Some('\\')),
            just('"').to(Some('"')),
            just('\'').to(Some('\'')),
            just('s').to(Some(' ')),
            just('x').ignore_then(uni(16, 2).map(Some)),
            uni(8, 3).map(Some),
            just('u').ignore_then(uni(16, 4).map(Some)),
            just('U').ignore_then(uni(16, 8).map(Some)),
            just('\n').then(inline_space).to(None),
        )))
        .boxed();
    let genqt = |q: char| {
        (none_of([q, '\\'])
            .or(esc.clone().filter(|x| x.is_some()).map(|x| x.unwrap()))
            .repeated())
        .collect::<String>()
        .delimited_by(just(q), just(format!("{q}\n")))
    };

    let s = (genqt('\'').or(genqt('\"')).then_ignore(inline_space)).boxed();

    (none_of("\n[]").repeated().collect::<String>()) // any chars but `[`, `]`, `\n`
        .delimited_by(just('['), just("]\n")) // section header
        .then_ignore(inline_space) // in case next line has whitespaces
        .then(
            text::ident()
                .then_ignore(just('=').padded())
                .then(
                    s.or(any::<_, Ext>().and_is(just("\\\n").or(just("\n")).not()).map(|x:char|x).repeated() // any chars except [\\]\n
                        .collect::<String>())
                        .separated_by(
                            (root_comments.then(just('\n')).or_not())
                                .delimited_by(just("\\\n"), inline_space).ignored(),
                        )
                            .collect::<Vec<String>>()
                        .map(|v| v.join(""))
                )
                .separated_by(newline_or_comment.clone())
                .allow_leading()
                .allow_trailing()
                .collect::<Vec<(String, String)>>(),
        )
        .separated_by(newline_or_comment.clone())
        .allow_leading()
        .collect()
        .then_ignore(end())
}

/// Parse a string into an intermediate representation of an INI file.
///
/// # Errors
///
/// Returns a list of errors if the input is not a valid Systemd unit format
pub fn parse_str(s: &str) -> Result<IniIntermediate, Err> {
    parser().parse(s).into_result()
}

#[cfg(test)]
mod tests {
    #[test]
    fn sanitytest_conf() {
        println!(
            "{:?}",
            super::parse_str(include_str!("../test/sanitytest.conf")).unwrap()
        );
    }
    #[allow(clippy::indexing_slicing)]
    #[test]
    fn quoting() {
        assert_eq!(
            super::parse_str(
                r#"
                [section]
                TheFunny="this is str"
            "#
            )
            .unwrap()["section"],
            vec![("TheFunny".into(), "this is str".into())]
        );
    }
}
