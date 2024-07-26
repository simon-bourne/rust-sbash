use nom::{
    branch::alt,
    bytes::complete::tag,
    character::{
        complete::{
            alpha1, alphanumeric1, line_ending, multispace1, none_of, not_line_ending, one_of,
            space0,
        },
        streaming::char,
    },
    combinator::{eof, map, opt, peek, recognize},
    error::{context, ErrorKind},
    multi::{many0, many1, many_till},
    sequence::{delimited, pair, preceded, tuple},
    Finish, IResult,
};
use nom_greedyerror::{convert_error, GreedyError};
use nom_locate::LocatedSpan;

use crate::{FnSignature, Item, ItemArg, ParseError, FORWARDED_ARGS_NAME};

pub fn parse(input: &str) -> Result<(Description, Vec<Item>), ParseError> {
    let input_span = Span::new(input);

    let (_, script) = match script(input_span).finish() {
        Ok(ok) => Ok(ok),
        Err(e) => Err(ParseError(convert_error(input, e))),
    }?;

    Ok(script)
}

fn script(input: Span) -> ParseResult<(Description, Vec<Item>)> {
    let (input, (script_docs, (items, _eof))) =
        context("script", pair(doc_comment('^'), many_till(ws(item), eof)))(input)?;
    Ok((input, (Description::new([&script_docs]), items)))
}

fn item(input: Span) -> ParseResult<Item> {
    let public = opt(ws(tag("pub")));
    let inline = opt(ws(tag("inline")));
    let body_block = ws(delimited(
        tuple((tag("{"), space0, opt(line_comment), many1(line_ending))),
        body,
        tag("}"),
    ));

    let (
        input,
        (pre_description, (is_pub, is_inline), _fn, (fn_signature, post_description, body)),
    ) = context(
        "function",
        tuple((
            doc_comment('>'),
            pair(public, inline),
            ws(tag("fn")),
            tuple((fn_signature, doc_comment('<'), body_block)),
        )),
    )(input)?;

    Ok((
        input,
        Item {
            description: Description::new([&pre_description, &post_description]),
            is_pub: is_pub.is_some(),
            is_inline: is_inline.is_some(),
            fn_signature,
            body: body.fragment(),
            body_line_number: body.location_line() as usize,
        },
    ))
}

fn fn_signature(input: Span) -> ParseResult<FnSignature> {
    let arg_list = pair(many0(ws(arg)), opt(ws(last_arg)));
    let (input, (name, (mut args, last_arg))) = context(
        "function signature",
        pair(
            text(identifier("_-")),
            ws(delimited(tag("("), arg_list, tag(")"))),
        ),
    )(input)?;

    let forward_extra_args = if let Some(last_arg) = last_arg {
        if last_arg.name == FORWARDED_ARGS_NAME {
            Some(last_arg.description)
        } else {
            args.push(last_arg);
            None
        }
    } else {
        None
    };

    Ok((
        input,
        FnSignature {
            name,
            args,
            forward_extra_args,
        },
    ))
}

fn arg(input: Span) -> ParseResult<ItemArg> {
    let (s, (pre_description, name, _comma, post_description)) = context(
        "argument",
        tuple((
            doc_comment('>'),
            text(identifier("_")),
            char(','),
            doc_comment('<'),
        )),
    )(input)?;

    item_arg(s, &pre_description, &post_description, name)
}

fn last_arg(input: Span) -> ParseResult<ItemArg> {
    let (s, (pre_description, name, _comma, post_description)) = context(
        "last argument",
        tuple((
            doc_comment('>'),
            text(alt((identifier("_"), tag(FORWARDED_ARGS_NAME)))),
            opt(char(',')),
            doc_comment('<'),
        )),
    )(input)?;

    item_arg(s, &pre_description, &post_description, name)
}

fn item_arg<'a>(
    input: Span<'a>,
    pre_description: &[Span<'a>],
    post_description: &[Span<'a>],
    name: &'a str,
) -> ParseResult<'a, ItemArg<'a>> {
    Ok((
        input,
        ItemArg {
            description: Description::new([pre_description, post_description]),
            name,
        },
    ))
}

fn body(input: Span) -> ParseResult<Span> {
    let (_, prefix) = space0(input)?;

    if prefix.is_empty() {
        return Ok((input, prefix));
    }

    let prefix = *prefix.fragment();

    context(
        "body",
        recognize(many0(pair(
            alt((recognize(tuple((tag(prefix), not_line_ending))), space0)),
            line_ending,
        ))),
    )(input)
}

fn identifier<'a>(seperator_chars: &'a str) -> impl FnMut(Span<'a>) -> ParseResult<'a, Span<'a>> {
    context(
        "identifier",
        recognize(pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, recognize(one_of(seperator_chars))))),
        )),
    )
}

fn text<'a>(
    parser: impl FnMut(Span<'a>) -> ParseResult<'a, Span<'a>>,
) -> impl FnMut(Span<'a>) -> ParseResult<'a, &'a str> {
    map(parser, |s| *s.fragment())
}

type Span<'a> = LocatedSpan<&'a str>;

type ParseResult<'a, T> = IResult<Span<'a>, T, GreedyError<Span<'a>, ErrorKind>>;

fn ws<'a, F, O>(inner: F) -> impl FnMut(Span<'a>) -> ParseResult<'a, O>
where
    F: FnMut(Span<'a>) -> ParseResult<'a, O> + 'a,
{
    delimited(ws_or_comments, inner, ws_or_comments)
}

fn ws_or_comments(input: Span) -> ParseResult<()> {
    let (s, _) = many0(alt((
        multispace1,
        recognize(tuple((line_comment, alt((line_ending, eof))))),
    )))(input)?;

    Ok((s, ()))
}

fn line_comment(input: Span) -> ParseResult<Span> {
    recognize(tuple((
        tag("#"),
        peek(alt((eof, recognize(none_of("><^"))))),
        not_line_ending,
    )))(input)
}

fn doc_comment<'a>(prefix: char) -> impl FnMut(Span<'a>) -> ParseResult<'a, Vec<Span<'a>>> {
    many0(ws(delimited(
        pair(char('#'), char(prefix)),
        preceded(space0, not_line_ending),
        alt((eof, line_ending)),
    )))
}

#[derive(Debug)]
pub struct Description {
    short: String,
    long: String,
}

impl Description {
    pub fn new<'a, const LEN: usize>(
        description: [impl IntoIterator<Item = &'a Span<'a>>; LEN],
    ) -> Self {
        let paragraphs: Vec<String> = description
            .into_iter()
            .flat_map(|lines| {
                let lines: Vec<&str> = lines.into_iter().map(|s| s.fragment().trim()).collect();

                lines
                    .split(|line| line.is_empty())
                    .map(|paragraph| paragraph.join(" "))
                    .collect::<Vec<_>>()
            })
            .filter(|para| !para.is_empty())
            .collect();

        let long = paragraphs.join("\n\n");

        Self {
            short: paragraphs.into_iter().next().unwrap_or_default(),
            long,
        }
    }

    pub fn short(&self) -> &str {
        &self.short
    }

    pub fn long(&self) -> &str {
        &self.long
    }
}
