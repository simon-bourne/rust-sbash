use itertools::Itertools;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{
    alpha1, alphanumeric1, line_ending, multispace0, not_line_ending, space0,
};
use nom::combinator::{opt, recognize};
use nom::error::ParseError;
use nom::multi::{many0, separated_list0};
use nom::sequence::{delimited, pair, preceded, separated_pair, tuple};
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
            "{}\n\nset -euo pipefail\n\n{} \"$@\"",
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
    is_pub: bool,
    is_inline: bool,
    ident: &'a str,
    body: &'a str,
}

// TODO: Make sure line numbers match up with bash line numbers
impl<'a> Item<'a> {
    fn script(&self) -> String {
        if self.body.is_empty() {
            format!("{} () {{ :; }}", self.ident)
        } else if self.is_inline {
            format!("{} () {{\n{}}}\n\n", self.ident, self.body)
        } else {
            format!("{} () {{ ( \n{} ) }}\n\n", self.ident, self.body)
        }
    }
}

fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
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

struct FnSignature<'a> {
    name: &'a str,
    args: Vec<&'a str>,
}

fn parse_fn_signature(input: &str) -> IResult<&str, FnSignature> {
    let (input, (name, args)) = pair(
        identifier,
        ws(delimited(
            tag("("),
            separated_list0(tag(","), ws(identifier)),
            tag(")"),
        )),
    )(input)?;

    Ok((input, FnSignature { name, args }))
}

fn parse_item(input: &str) -> IResult<&str, Item> {
    let (input, ((is_pub, is_inline), (ident, body))) = separated_pair(
        pair(opt(ws(tag("pub"))), opt(ws(tag("inline")))),
        ws(tag("fn")),
        tuple((
            identifier,
            ws(delimited(
                tuple((tag("{"), space0, line_ending)),
                parse_body,
                tag("}"),
            )),
        )),
    )(input)?;

    Ok((
        input,
        Item {
            is_pub: is_pub.is_some(),
            is_inline: is_inline.is_some(),
            ident,
            body,
        },
    ))
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

    // TODO: Can we make a temporary file for the script so bash can read stdin?
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
