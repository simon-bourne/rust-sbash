use itertools::Itertools;
use std::{
    env,
    error::Error,
    fs,
    io::Write,
    iter,
    ops::Range,
    os::unix::prelude::CommandExt,
    process::{self, Command, Stdio},
};

use logos::{Lexer, Logos};

#[derive(Logos, Debug, PartialEq)]
enum Token {
    #[token("pub")]
    Pub,

    #[token("inline")]
    Inline,

    #[token("fn")]
    Fn,

    #[token("(")]
    OpenBracket,

    #[token(")")]
    CloseBracket,

    #[token("{")]
    OpenBrace,

    #[token("}")]
    CloseBrace,

    #[token(",")]
    Comma,

    #[regex("a-zA-Z[a-zA-Z0-9_]*")]
    Identifier,

    #[error]
    #[regex(r"[ \t\r\n\f]+", logos::skip)]
    Error,
}

struct TokenStream<'a> {
    lines: &'a [&'a str],
    line_number: usize,
    lex: Lexer<'a, Token>,
}

impl<'a> TokenStream<'a> {
    fn new(lines: &'a [&'a str]) -> Self {
        let lex = Token::lexer(lines[0]);
        Self {
            lines,
            line_number: 0,
            lex,
        }
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.lex.next();

        if token.is_some() {
            token
        } else {
            let line_number = self.next_line();

            if line_number < self.lines.len() {
                self.lex = Token::lexer(self.line());
                self.next()
            } else {
                None
            }
        }
    }

    fn span(&self) -> Range<usize> {
        self.lex.span()
    }

    fn body(&mut self) -> Vec<&'a str> {
        if let token = self.lex.next() {
            panic!("Unexpected token before body");
        }

        let mut body = Vec::new();

        loop {
            let line_number = self.next_line();

            if line_number < self.lines.len() {
                let line = self.line();

                if line.starts_with(' ') {
                    body.push(line);
                } else {
                    self.lex = Token::lexer(line);
                    return body;
                }
            } else {
                panic!("Unexpected end of file");
            }
        }
    }

    fn next_line(&mut self) -> usize {
        self.line_number += 1;
        self.line_number
    }

    fn line(&self) -> &'a str {
        self.lines[self.line_number]
    }
}

struct IsInline(bool);
struct IsPub(bool);

fn script(tokens: &mut TokenStream) {
    while let Some(token) = tokens.next() {
        match token {
            Token::Pub => inline_function(tokens, IsPub(true)),
            Token::Inline => {
                expect(tokens, Token::Fn);
                function(tokens, IsInline(true), IsPub(false))
            }
            Token::Fn => function(tokens, IsInline(false), IsPub(false)),
            Token::Error => panic!("Error"),
            _ => panic!("Unexpected token {:?}", token),
        }
    }
}

fn inline_function(tokens: &mut TokenStream, public: IsPub) {
    if let Some(token) = tokens.next() {
        match token {
            Token::Inline => {
                expect(tokens, Token::Fn);
                function(tokens, IsInline(true), public)
            }
            Token::Fn => function(tokens, IsInline(false), public),
            Token::Error => panic!("Error"),
            _ => panic!("Unexpected token {:?}", token),
        }
    }
}

fn expect(tokens: &mut TokenStream, token: Token) {
    assert!(tokens.next().unwrap() == token);
}

fn function(tokens: &mut TokenStream, inline: IsInline, public: IsPub) {
    if let Some(token) = tokens.next() {
        match token {
            Token::OpenBrace => {
                tokens.body();
            }
            Token::Error => panic!("Error"),
            _ => panic!("Unexpected token {:?}", token),
        }
    }
}

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

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();
    args.next();
    let script_file = args.next().unwrap();
    // TODO: Default to looking for main
    let function = args.next();
    let input = fs::read_to_string(&script_file)?;

    // TODO: Parse error handling
    let items: Script = todo!();
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
