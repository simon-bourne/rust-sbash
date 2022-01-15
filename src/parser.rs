use nom::{
    branch::alt,
    bytes::complete::tag,
    character::{
        complete::{
            alpha1, alphanumeric1, line_ending, multispace1, none_of, not_line_ending, space0,
        },
        streaming::char,
    },
    combinator::{eof, map, opt, peek, recognize},
    error::{context, ErrorKind},
    multi::{many0, many1, many_till, separated_list0},
    sequence::{delimited, pair, tuple},
    Finish, IResult,
};
use nom_greedyerror::{convert_error, GreedyError};
use nom_locate::LocatedSpan;

use crate::{FnSignature, Item, ParseError};

pub fn parse(input: &str) -> Result<Vec<Item>, ParseError> {
    let input_span = Span::new(input);

    let (_, items) = match script(input_span).finish() {
        Ok(ok) => Ok(ok),
        Err(e) => Err(ParseError(convert_error(input, e))),
    }?;

    Ok(items)
}

fn script(input: Span) -> ParseResult<Vec<Item>> {
    let (input, (items, _eof)) = many_till(ws(item), eof)(input)?;
    Ok((input, items))
}

fn item(input: Span) -> ParseResult<Item> {
    let (input, (description, (is_pub, is_inline), _, (fn_signature, body))) = context(
        "function",
        tuple((
            doc_comment('>'),
            pair(opt(ws(tag("pub"))), opt(ws(tag("inline")))),
            ws(tag("fn")),
            tuple((
                fn_signature,
                ws(delimited(
                    tuple((tag("{"), space0, opt(line_comment), many1(line_ending))),
                    body,
                    tag("}"),
                )),
            )),
        )),
    )(input)?;

    Ok((
        input,
        Item {
            description: description.map(|s| *s.fragment()),
            is_pub: is_pub.is_some(),
            is_inline: is_inline.is_some(),
            fn_signature,
            body: body.fragment(),
            body_line_number: body.location_line() as usize,
        },
    ))
}

fn fn_signature(input: Span) -> ParseResult<FnSignature> {
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

fn body(input: Span) -> ParseResult<Span> {
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

fn identifier(input: Span) -> ParseResult<Span> {
    context(
        "identifier",
        recognize(pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, tag("_")))),
        )),
    )(input)
}

fn text<'a>(
    parser: impl FnMut(Span<'a>) -> ParseResult<'a, Span<'a>>,
) -> impl FnMut(Span<'a>) -> ParseResult<'a, &'a str> {
    map(parser, |s| *s.fragment())
}

type Span<'a> = LocatedSpan<&'a str>;

type ParseResult<'a, T> = IResult<Span<'a>, T, GreedyError<Span<'a>, ErrorKind>>;

fn ws<'a, F: 'a, O>(inner: F) -> impl FnMut(Span<'a>) -> ParseResult<'a, O>
where
    F: FnMut(Span<'a>) -> ParseResult<'a, O>,
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

fn doc_comment<'a>(prefix: char) -> impl FnMut(Span<'a>) -> ParseResult<'a, Option<Span<'a>>> {
    // TODO: Trim and combine many doc comments
    opt(ws(delimited(
        pair(char('#'), char(prefix)),
        not_line_ending,
        alt((eof, line_ending)),
    )))
}
