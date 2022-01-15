use std::{
    collections::HashMap,
    fmt::{self, Display},
};

use clap::{App, Arg};
use thiserror::Error;

mod parser;

#[derive(Debug)]
pub struct Script<'a> {
    items: Vec<Item<'a>>,
}

impl<'a> Script<'a> {
    pub fn parse(input: &'a str) -> Result<Self, ParseError> {
        parser::parse(input)
    }

    // TODO: Error type for return?
    pub fn parse_args(
        &self,
        exe_name: &str,
        args: impl IntoIterator<Item = String>,
    ) -> Result<(String, Vec<String>), String> {
        let mut app = App::new(exe_name);
        let mut name_to_args = HashMap::new();

        let mut main_is_pub = None;

        for item in &self.items {
            let name = item.fn_signature.name;

            if item.is_pub {
                let mut subcmd = App::new(name);
                let mut arg_names = Vec::new();

                for &arg in &item.fn_signature.args {
                    subcmd = subcmd.arg(Arg::new(arg));
                    arg_names.push(arg);
                }

                let name_exists = name_to_args.insert(name, arg_names).is_some();

                if name_exists {
                    return Err(format!("Duplicate function name {}", name));
                }

                app = app.subcommand(subcmd);
            }

            (name == "main").then(|| main_is_pub = Some(item.is_pub));
        }

        extract_args(app, args, name_to_args)
    }
}

fn extract_args(
    app: App,
    args: impl IntoIterator<Item = String>,
    mut name_to_args: HashMap<&str, Vec<&str>>,
) -> Result<(String, Vec<String>), String> {
    let arg_matches = app.get_matches_from(args);

    match arg_matches.subcommand() {
        Some((name, subcmd_matches)) => {
            let arg_values = name_to_args
                .remove(name)
                .unwrap()
                .into_iter()
                .map(|arg_name| {
                    let mut values = subcmd_matches.values_of(arg_name).unwrap();
                    let value = values.next().unwrap();
                    assert!(values.next().is_none());

                    value.to_owned()
                })
                .collect();

            Ok((name.to_owned(), arg_values))
        }
        None => todo!(),
    }
}

impl<'a> Display for Script<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut script = String::new();

        for item in &self.items {
            script.push_str(&item.script(count_newlines(&script)));
        }

        write!(f, "{}", script)
    }
}

#[derive(Error, Debug)]
#[error("Parse error:\n{0}")]
pub struct ParseError(String);

fn count_newlines(s: &str) -> usize {
    bytecount::count(s.as_bytes(), b'\n')
}

#[derive(Debug)]
pub struct Item<'a> {
    is_pub: bool,
    is_inline: bool,
    fn_signature: FnSignature<'a>,
    body: &'a str,
    body_line_number: usize,
}

impl<'a> Item<'a> {
    fn script(&self, newline_count: usize) -> String {
        let name = self.fn_signature.name;
        let current_line = newline_count + 1;
        let current_body_line = current_line + 1;
        let desired_body_line = self.body_line_number;

        assert!(desired_body_line >= current_body_line);
        let extra_newlines = "\n".repeat(desired_body_line - current_body_line);

        if self.body.is_empty() {
            format!("{}{} () {{ :; }}", extra_newlines, name)
        } else if self.is_inline {
            format!(
                "{}{} () {{ {}\n{}}};",
                extra_newlines,
                name,
                self.fn_signature.args(),
                self.body
            )
        } else {
            format!(
                "{}{} () {{ ( {}\n{}) }};",
                extra_newlines,
                name,
                self.fn_signature.args(),
                self.body
            )
        }
    }
}

#[derive(Debug)]
pub struct FnSignature<'a> {
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
