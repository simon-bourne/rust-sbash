use nom::branch::alt;
use nom::bytes::complete::{tag, take_till};
use nom::character::complete::{
    alpha1, alphanumeric1, char, line_ending, multispace0, multispace1, not_line_ending,
};
use nom::combinator::recognize;
use nom::error::ParseError;
use nom::multi::many0;
use nom::sequence::{delimited, pair, tuple};
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

fn ws<'a, F: 'a, O, E: ParseError<&'a str>>(
    inner: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: FnMut(&'a str) -> IResult<&'a str, O, E>,
{
    delimited(multispace0, inner, multispace0)
}

fn parse_comment(input: &str) -> IResult<&str, &str> {
    ws(delimited(char('#'), not_line_ending, line_ending))(input)
}

fn parse_comments(input: &str) -> IResult<&str, Vec<&str>> {
    many0(parse_comment)(input)
}

#[derive(Debug)]
struct Item<'a> {
    ident: &'a str,
}

pub fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"), tag("."))),
        many0(alt((alphanumeric1, tag("_")))),
    ))(input)
}

fn parse_item(input: &str) -> IResult<&str, Item> {
    let (input, (_, _, ident)) = ws(tuple((tag("fn"), multispace1, identifier)))(input)?;

    Ok((input, Item { ident }))
}

fn parse<'a>(input: &'a str) -> IResult<&'a str, Script> {
    let (input, items) = many0(parse_item)(input)?;
    Ok((input, Script { items }))
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();
    args.next();
    let script_file = args.next().unwrap();
    let input = fs::read_to_string(&script_file)?;

    // TODO: Parse error handling
    let (_, items) = parse(&input).unwrap();
    println!("{:?}", items);

    process::exit(0);

    let mut child = Command::new("bash")
        .arg0(script_file)
        .args(iter::once("-s".to_owned()).chain(args))
        .stdin(Stdio::piped())
        .spawn()?;


    child.stdin.as_mut().unwrap().write_all(input.as_bytes())?;

    // TODO: Is this OK? Do zombies get cleaned up when we exit?
    match child.wait()?.code() {
        Some(code) => process::exit(code),
        None => panic!("Process terminated by signal"),
    }
}
