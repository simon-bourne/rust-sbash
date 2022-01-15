use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{
        alpha1, alphanumeric1, line_ending, multispace0, not_line_ending, space0,
    },
    combinator::{eof, map, opt, recognize},
    error::{context, ErrorKind},
    multi::{many0, many_till, separated_list0},
    sequence::{delimited, pair, separated_pair, tuple},
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
    let (input, ((is_pub, is_inline), (fn_signature, body))) = context(
        "function",
        separated_pair(
            pair(opt(ws(tag("pub"))), opt(ws(tag("inline")))),
            ws(tag("fn")),
            tuple((
                fn_signature,
                ws(delimited(
                    tuple((tag("{"), space0, line_ending)),
                    body,
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
    delimited(multispace0, inner, multispace0)
}
