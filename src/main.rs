use std::{
    env,
    error::Error,
    fs,
    io::Write,
    iter,
    os::unix::prelude::CommandExt,
    process::{self, Command, Stdio},
};

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{
        alpha1, alphanumeric1, line_ending, multispace0, not_line_ending, space0,
    },
    combinator::{eof, map, opt, recognize},
    error::{context, convert_error, VerboseError},
    multi::{many0, many_till, separated_list0},
    sequence::{delimited, pair, separated_pair, tuple},
    Finish, IResult,
};
use nom_locate::LocatedSpan;

#[derive(Debug)]
struct Script<'a> {
    items: Vec<Item<'a>>,
}

fn count_newlines(s: &str) -> usize {
    bytecount::count(s.as_bytes(), b'\n')
}

impl<'a> Script<'a> {
    fn script(&self, function: &Option<impl AsRef<str>>) -> String {
        if let Some(function) = function.as_ref() {
            let mut script = String::new();

            for item in &self.items {
                script.push_str(&item.script(count_newlines(&script)));
            }

            script.push_str(&format!(
                "\n\nset -euo pipefail\n\n{} \"$@\"",
                function.as_ref()
            ));

            script
        } else {
            "".to_owned()
        }
    }
}

type Span<'a> = LocatedSpan<&'a str>;

type ParseResult<'a, T> = IResult<Span<'a>, T, VerboseError<Span<'a>>>;

fn ws<'a, F: 'a, O>(inner: F) -> impl FnMut(Span<'a>) -> ParseResult<'a, O>
where
    F: FnMut(Span<'a>) -> ParseResult<'a, O>,
{
    delimited(multispace0, inner, multispace0)
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
    fn script(&self, newline_count: usize) -> String {
        let name = self.fn_signature.name;
        let current_line = newline_count + 1;
        let current_body_line = current_line + 1;
        let desired_body_line = self.body.location_line() as usize;

        assert!(desired_body_line >= current_body_line);
        let extra_newlines = "\n".repeat(desired_body_line - current_body_line);

        if self.body.is_empty() {
            format!("{}{} () {{ :; }}", extra_newlines, name)
        } else if self.is_inline {
            format!(
                "{}{} () {{ {} # Line {} \n{}}};",
                extra_newlines,
                name,
                self.fn_signature.args(),
                self.body.location_line(),
                self.body.fragment()
            )
        } else {
            format!(
                "{}{} () {{ ( {} # Line {} \n{} ) }};",
                extra_newlines,
                name,
                self.fn_signature.args(),
                self.body.location_line(),
                self.body.fragment()
            )
        }
    }
}

fn identifier(input: Span) -> ParseResult<Span> {
    context(
        "identifier",
        recognize(pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, tag("_")))),
        )),
    )(input)
}

// TODO: Type alias for parser? Or is there one in nom?
fn text<'a>(
    parser: impl FnMut(Span<'a>) -> ParseResult<'a, Span<'a>>,
) -> impl FnMut(Span<'a>) -> ParseResult<'a, &'a str> {
    map(parser, |s| *s.fragment())
}

fn parse_body(input: Span) -> ParseResult<Span> {
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

fn parse_fn_signature(input: Span) -> ParseResult<FnSignature> {
    let (input, (name, args)) = context(
        "function signature",
        pair(
            text(identifier),
            ws(delimited(
                tag("("),
                separated_list0(tag(","), ws(text(identifier))),
                tag(")"),
            )),
        ),
    )(input)?;

    Ok((input, FnSignature { name, args }))
}

fn parse_item(input: Span) -> ParseResult<Item> {
    let (input, ((is_pub, is_inline), (fn_signature, body))) = context(
        "function",
        separated_pair(
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
        ),
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

fn parse(input: Span) -> ParseResult<Script> {
    let (input, (items, _eof)) = many_till(ws(parse_item), eof)(input)?;
    Ok((input, Script { items }))
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();
    args.next();
    let script_file = args.next().unwrap();
    let function = args.next();
    let input = fs::read_to_string(&script_file)?;

    // TODO: Parse error handling
    let input_span = Span::new(&input);

    let (_, items) = match parse(input_span).finish() {
        Ok(ok) => Ok(ok),
        Err(e) => {
            // See <https://github.com/fflorent/nom_locate/issues/36>
            let errors: Vec<_> = e
                .errors
                .into_iter()
                .map(|(input, error)| (*input.fragment(), error))
                .collect();

            Err(convert_error(input.as_str(), VerboseError { errors }))
        }
    }?;

    let script = items.script(&function);
    println!("{}", script);

    // TODO: Can we make a temporary file for the script so bash can read stdin?
    let mut child = Command::new("bash")
        .arg0(script_file)
        .args(iter::once("-s".to_owned()).chain(args))
        .stdin(Stdio::piped())
        .spawn()?;

    let wrote_stdin = child.stdin.as_mut().unwrap().write_all(script.as_bytes());

    match wrote_stdin {
        Ok(_) => match child.wait()?.code() {
            Some(code) => process::exit(code),
            None => panic!("Process terminated by signal"),
        },
        Err(e) => {
            // Kill the child and reap the process handle
            child.kill().ok();
            child.wait().ok();
            Err(e)
        }
    }?;

    Ok(())
}

fn main() {
    match run() {
        Ok(_) => (),
        Err(e) => println!("{}", e),
    }
}
