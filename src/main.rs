use itertools::Itertools;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{
    alpha1, alphanumeric1, line_ending, multispace0, not_line_ending, space0,
};
use nom::combinator::recognize;
use nom::error::ParseError;
use nom::multi::many0;
use nom::sequence::{delimited, pair, preceded, tuple};
use nom::IResult;
use std::{
    env,
    error::Error,
    fs,
    io::Write,
    iter,
    os::unix::prelude::CommandExt,
    process::{self, Command, Stdio},
};

#[derive(Debug)]
struct Script<'a> {
    items: Vec<Item<'a>>,
}

impl<'a> Script<'a> {
    fn script(&self, function: &Option<impl AsRef<str>>) -> String {
        let function = function.as_ref().map_or("main", AsRef::as_ref);
        format!(
            "{}\n\n{} \"$@\"",
            self.items.iter().map(Item::script).join("\n"),
            function
        )
    }
}

fn ws<'a, F: 'a, O, E: ParseError<&'a str>>(
    inner: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: FnMut(&'a str) -> IResult<&'a str, O, E>,
{
    delimited(multispace0, inner, multispace0)
}

fn parse_comment(input: &str) -> IResult<&str, &str> {
    preceded(pair(tag("#"), space0), not_line_ending)(input)
}

fn parse_comments(input: &str) -> IResult<&str, Vec<&str>> {
    many0(parse_comment)(input)
}

#[derive(Debug)]
struct Item<'a> {
    ident: &'a str,
    body: &'a str,
}

// TODO: Make sure line numbers match up with bash line numbers
impl<'a> Item<'a> {
    fn script(&self) -> String {
        if self.body.is_empty() {
            format!("{} () {{ :; }}", self.ident)
        } else {
            format!(
                "{} () {{ ( set -euo pipefail \n{}) }}\n\n",
                self.ident, self.body
            )
        }
    }
}

fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_"), tag(".")))),
    ))(input)
}

fn parse_body(input: &str) -> IResult<&str, &str> {
    let (_, prefix) = space0(input)?;

    if prefix.is_empty() {
        return Ok((input, ""));
    }

    recognize(many0(pair(
        alt((recognize(tuple((tag(prefix), not_line_ending))), space0)),
        line_ending,
    )))(input)
}

fn parse_item(input: &str) -> IResult<&str, Item> {
    let (input, (ident, body)) = preceded(
        tag("fn"),
        tuple((
            ws(identifier),
            delimited(tuple((tag("{"), space0, line_ending)), parse_body, tag("}")),
        )),
    )(input)?;

    Ok((input, Item { ident, body }))
}

fn parse(input: &str) -> IResult<&str, Script> {
    let (input, items) = many0(ws(parse_item))(input)?;
    Ok((input, Script { items }))
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();
    args.next();
    let script_file = args.next().unwrap();
    // TODO: Default to looking for main
    let function = args.next();
    let input = fs::read_to_string(&script_file)?;

    // TODO: Parse error handling
    let (_, items) = parse(&input).unwrap();
    let script = items.script(&function);
    println!("{}", script);

    let mut child = Command::new("bash")
        .arg0(script_file)
        .args(iter::once("-s".to_owned()).chain(args))
        .stdin(Stdio::piped())
        .spawn()?;

    child.stdin.as_mut().unwrap().write_all(script.as_bytes())?;

    // TODO: Is this OK? Do zombies get cleaned up when we exit?
    match child.wait()?.code() {
        Some(code) => process::exit(code),
        None => panic!("Process terminated by signal"),
    }
}
