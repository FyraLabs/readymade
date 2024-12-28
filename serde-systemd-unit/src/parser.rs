use chumsky::prelude::*;
use std::collections::HashMap;

pub(crate) type IniIntermediate = HashMap<String, Vec<(String, String)>>;
pub(crate) type Err = Vec<chumsky::error::Simple<char>>;

#[must_use]
fn parser() -> impl Parser<char, IniIntermediate, Error = Simple<char>> {
    let empty_lines = just('\n')
        .then(filter(|c: &char| c.is_whitespace() && *c != '\n').repeated())
        .repeated()
        .ignored();
    let root_comments = (just('#').or(just(';')))
        .then(none_of('\n').repeated())
        .separated_by(empty_lines);
    let newline_or_comment = root_comments
        .clone()
        .allow_leading()
        .allow_trailing()
        .ignored()
        .or(empty_lines);

    (none_of("\n[]").repeated().collect::<String>())
        .delimited_by(just('['), just("]\n"))
        .then_ignore(filter(|c: &char| c.is_whitespace() && *c != '\n').repeated())
        .then_ignore(newline_or_comment.clone())
        .then(
            text::ident()
                .then_ignore(just('=').padded())
                .then(
                    (just("\\\n").or(just("\n")).not().repeated())
                        .separated_by(root_comments.then(just('\n')).or_not().delimited_by(
                            just("\\\n"),
                            filter(|&c: &char| c.is_whitespace() && c != '\n').repeated(),
                        ))
                        .flatten()
                        .collect::<String>(),
                )
                .separated_by(newline_or_comment.clone())
                .allow_leading()
                .allow_trailing()
                .collect::<Vec<(_, _)>>(),
        )
        .separated_by(newline_or_comment.clone())
        .allow_leading()
        .then_ignore(newline_or_comment)
        .collect()
        .then_ignore(end())
}

pub fn parse_str(s: &str) -> Result<IniIntermediate, Err> {
    parser().parse(s)
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
}
