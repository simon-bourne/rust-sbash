use std::{
    env,
    error::Error,
    fs,
    io::Write,
    iter,
    os::unix::prelude::CommandExt,
    process::{self, Command, Stdio},
};

use itertools::Itertools;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{
        alpha1, alphanumeric1, line_ending, multispace0, not_line_ending, space0,
    },
    combinator::{map, opt, recognize},
    error::ParseError,
    multi::{many0, separated_list0},
    sequence::{delimited, pair, preceded, separated_pair, tuple},
    IResult,
};
use nom_locate::LocatedSpan;

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

type Span<'a> = LocatedSpan<&'a str>;

fn ws<'a, F: 'a, O, E: ParseError<Span<'a>>>(
    inner: F,
) -> impl FnMut(Span<'a>) -> IResult<Span<'a>, O, E>
where
    F: FnMut(Span<'a>) -> IResult<Span<'a>, O, E>,
{
    delimited(multispace0, inner, multispace0)
}

fn parse_comment(input: Span) -> IResult<Span, Span> {
    preceded(pair(tag("#"), space0), not_line_ending)(input)
}

fn parse_comments(input: Span) -> IResult<Span, Vec<Span>> {
    many0(parse_comment)(input)
}

#[derive(Debug)]
struct Item<'a> {
    is_pub: bool,
    is_inline: bool,
    fn_signature: FnSignature<'a>,
    body: Span<'a>,
}

// TODO: Make sure line numbers match up with bash line numbers
// TODO: Rather than `script` method, use formatting
impl<'a> Item<'a> {
    fn script(&self) -> String {
        let name = self.fn_signature.name;

        if self.body.is_empty() {
            format!("{} () {{ :; }}", name)
        } else if self.is_inline {
            format!(
                "{} () {{ {} # Line {} \n{}}}\n\n",
                name,
                self.fn_signature.args(),
                self.body.location_line(),
                self.body.fragment()
            )
        } else {
            format!(
                "{} () {{ ( {} # Line {} \n{} ) }}\n\n",
                name,
                self.fn_signature.args(),
                self.body.location_line(),
                self.body.fragment()
            )
        }
    }
}

fn identifier(input: Span) -> IResult<Span, Span> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    ))(input)
}

// TODO: Type alias for parser? Or is there one in nom?
fn text<'a>(
    parser: impl FnMut(Span<'a>) -> IResult<Span<'a>, Span<'a>>,
) -> impl FnMut(Span<'a>) -> IResult<Span<'a>, &'a str> {
    map(parser, |s| *s.fragment())
}

fn parse_body(input: Span) -> IResult<Span, Span> {
    let (_, prefix) = space0(input)?;

    if prefix.is_empty() {
        return Ok((input, prefix));
    }

    let prefix = *prefix.fragment();

    recognize(many0(pair(
        alt((recognize(tuple((tag(prefix), not_line_ending))), space0)),
        line_ending,
    )))(input)
}

#[derive(Debug)]
struct FnSignature<'a> {
    name: &'a str,
    args: Vec<&'a str>,
}

impl<'a> FnSignature<'a> {
    fn args(&self) -> String {
        let mut arg_str = String::new();

        for arg in &self.args {
            arg_str.push_str(&format!("{}=\"$1\"; shift; ", arg));
        }

        arg_str
    }
}

fn parse_fn_signature(input: Span) -> IResult<Span, FnSignature> {
    let (input, (name, args)) = pair(
        text(identifier),
        ws(delimited(
            tag("("),
            separated_list0(tag(","), ws(text(identifier))),
            tag(")"),
        )),
    )(input)?;

    Ok((input, FnSignature { name, args }))
}

fn parse_item(input: Span) -> IResult<Span, Item> {
    let (input, ((is_pub, is_inline), (fn_signature, body))) = separated_pair(
        pair(opt(ws(tag("pub"))), opt(ws(tag("inline")))),
        ws(tag("fn")),
        tuple((
            parse_fn_signature,
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
            fn_signature,
            body,
        },
    ))
}

fn parse(input: Span) -> IResult<Span, Script> {
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
    let (_, items) = parse(Span::new(&input)).unwrap();
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
