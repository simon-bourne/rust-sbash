use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Display},
    io, process,
};

use clap::{App, Arg, ArgEnum, ArgMatches};
use clap_complete::{generate, Shell};
use indoc::indoc;
use parser::Description;
use thiserror::Error;

mod parser;

#[derive(Debug)]
pub struct Script<'a> {
    description: Description,
    items: Vec<Item<'a>>,
}

impl<'a> Script<'a> {
    pub fn parse(input: &'a str) -> Result<Self, ParseError> {
        let (description, items) = parser::parse(input)?;
        let mut names = HashSet::new();

        for item in &items {
            let name = item.fn_signature.name;

            assert!(names.insert(name));
        }

        Ok(Self { description, items })
    }

    pub fn parse_args(&self, script_file: &str, args: impl IntoIterator<Item = String>) -> Action {
        let shell_comp_help = format!(
            indoc! {r#"
                Generate shell completions. To generate bash completions:

                source <("{}" --shell-completions bash)
            "#},
            script_file
        );
        let app = App::new(script_file)
            .about(self.description.short())
            .long_about(self.description.long())
            .arg(
                Arg::new(SHOW_SCRIPT_FLAG)
                    .long(SHOW_SCRIPT_FLAG)
                    .takes_value(false)
                    .help("Show the generated bash script (without subcommand code)"),
            )
            .arg(
                Arg::new(DEBUG_FLAG)
                    .long(DEBUG_FLAG)
                    .takes_value(false)
                    .exclusive(true)
                    .help("Show the generated bash script for a subcommand"),
            )
            .arg(
                Arg::new(SHELL_COMPLETIONS)
                    .long(SHELL_COMPLETIONS)
                    .help("Generate shell completions")
                    .long_help(shell_comp_help.as_str())
                    .possible_values(
                        Shell::value_variants()
                            .iter()
                            .filter_map(ArgEnum::to_possible_value),
                    )
                    .exclusive(true),
            );

        let mut app = app;
        let mut name_to_args = HashMap::new();

        for item in &self.items {
            let name = item.fn_signature.name;

            if item.is_pub {
                let description = &item.description;
                let subcmd_app = App::new(name)
                    .about(description.short())
                    .long_about(description.long());

                let (subcmd, arg_names) = item_arg_spec(subcmd_app, item);

                name_to_args.insert(name, arg_names);
                app = app.subcommand(subcmd);
            }
        }

        let arg_matches = app
            .try_get_matches_from_mut(args)
            .unwrap_or_else(|e| e.exit());

        if let Ok(generator) = arg_matches.value_of_t::<Shell>(SHELL_COMPLETIONS) {
            generate(generator, &mut app, script_file, &mut io::stdout());
            process::exit(0);
        } else if arg_matches.is_present(SHOW_SCRIPT_FLAG) {
            Action::ShowScript
        } else if let Some((name, subcmd_matches)) = arg_matches.subcommand() {
            let arg_names = name_to_args.remove(name).unwrap();

            Action::FnCall {
                name: name.to_owned(),
                args: extract_args(subcmd_matches, arg_names),
                debug: arg_matches.is_present(DEBUG_FLAG),
            }
        } else {
            app.print_help().unwrap();
            process::exit(2);
        }
    }
}

const SHOW_SCRIPT_FLAG: &str = "show-script";
const DEBUG_FLAG: &str = "debug";
const SHELL_COMPLETIONS: &str = "shell-completions";

pub enum Action {
    FnCall {
        name: String,
        args: Vec<String>,
        debug: bool,
    },
    ShowScript,
}

fn item_arg_spec<'a>(mut app: App<'a>, item: &'a Item) -> (App<'a>, ItemArgNames<'a>) {
    let mut names = Vec::new();

    for item_arg in &item.fn_signature.args {
        let description = &item_arg.description;
        let arg = Arg::new(item_arg.name)
            .required(true)
            .multiple_values(false)
            .help(description.short())
            .long_help(description.long());
        app = app.arg(arg);
        names.push(item_arg.name);
    }

    let forward_extra_args = &item.fn_signature.forward_extra_args;

    if let Some(description) = forward_extra_args {
        let arg = Arg::new(FORWARDED_ARGS_NAME)
            .required(false)
            .multiple_values(true)
            .help(description.short())
            .long_help(description.long());
        app = app.arg(arg);
    }

    (
        app,
        ItemArgNames {
            names,
            forward_extra: forward_extra_args.is_some(),
        },
    )
}

struct ItemArgNames<'a> {
    names: Vec<&'a str>,
    forward_extra: bool,
}

fn extract_args(arg_matches: &ArgMatches, item_args: ItemArgNames) -> Vec<String> {
    let positional_args = item_args.names.into_iter().map(|item_arg| {
        let mut values = arg_matches.values_of(item_arg).unwrap();
        let value = values.next().unwrap();
        assert!(values.next().is_none());

        value
    });

    let forwarded_args = item_args
        .forward_extra
        .then(|| {
            arg_matches
                .values_of(FORWARDED_ARGS_NAME)
                .into_iter()
                .flatten()
        })
        .into_iter()
        .flatten();

    positional_args
        .chain(forwarded_args)
        .map(|arg| arg.to_owned())
        .collect()
}

const FORWARDED_ARGS_NAME: &str = "$@";

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

impl ParseError {
    pub fn text(&self) -> &str {
        &self.0
    }
}

fn count_newlines(s: &str) -> usize {
    bytecount::count(s.as_bytes(), b'\n')
}

#[derive(Debug)]
pub struct Item<'a> {
    description: Description,
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
struct FnSignature<'a> {
    name: &'a str,
    args: Vec<ItemArg<'a>>,
    forward_extra_args: Option<Description>,
}

impl<'a> FnSignature<'a> {
    fn args(&self) -> String {
        let mut arg_str = String::new();

        for arg in &self.args {
            arg_str.push_str(&format!("{}=\"$1\"; shift; ", arg.name));
        }

        arg_str
    }
}

#[derive(Debug)]
struct ItemArg<'a> {
    name: &'a str,
    description: Description,
}
