use chumsky::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub enum Value {
    Number(usize),
    UnquotedText(String),
    QuotedText(String),
    #[default]
    Nil,
}

#[derive(Clone, Debug, Default)]
pub struct Section {
    pub name: String,
    /// TODO: possibly migrate to `HashMap<String, Value>`
    pub entries: HashMap<String, String>,
}

impl From<(String, HashMap<String, String>)> for Section {
    fn from((name, entries): (String, HashMap<String, String>)) -> Self {
        Self { name, entries }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Config(Vec<Section>);

impl std::iter::FromIterator<(String, HashMap<String, String>)> for Config {
    fn from_iter<T: IntoIterator<Item = (String, HashMap<String, String>)>>(iter: T) -> Self {
        Self(iter.into_iter().map(Section::from).collect())
    }
}

impl Config {
    #[must_use]
    pub fn parser() -> impl Parser<char, Self, Error = Simple<char>> {
        let root_comments = just('#')
            .or(just(';'))
            .then(none_of('\n').repeated())
            .separated_by(just('\n'));
        let comment = root_comments
            .clone()
            .allow_leading()
            .allow_trailing()
            .ignored();
        let line_escape = root_comments.then(just('\n')).or_not().delimited_by(
            just("\\\n"),
            filter(|&c: &char| c.is_whitespace() && c != '\n').repeated(),
        );
        let newline_or_comment = comment.or(just('\n').repeated().ignored());
        none_of("\n[]")
            .repeated()
            .collect::<String>()
            .delimited_by(just('['), just(']'))
            .then_ignore(just('\n').repeated())
            .then(
                text::ident()
                    .then_ignore(just('='))
                    .then(
                        just("\\\n")
                            .or(just("\n"))
                            .not()
                            .repeated()
                            .separated_by(line_escape)
                            .flatten()
                            .collect::<String>(),
                    )
                    .separated_by(newline_or_comment.clone())
                    .allow_leading()
                    .allow_trailing()
                    .collect::<HashMap<_, _>>(),
            )
            .separated_by(newline_or_comment)
            .allow_leading()
            .collect()
            .then_ignore(end())
    }
}

impl std::str::FromStr for Config {
    type Err = Vec<chumsky::error::Simple<char>>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parser().parse(s)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn sanitytest_conf() {
        println!(
            "{:?}",
            Config::from_str(include_str!("../test/sanitytest.conf")).unwrap()
        );
    }
}
