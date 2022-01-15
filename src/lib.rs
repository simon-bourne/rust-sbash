use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Display},
};

use clap::{App, AppSettings, Arg, ArgMatches};
use thiserror::Error;

mod parser;

#[derive(Debug)]
pub struct Script<'a> {
    items: Vec<Item<'a>>,
    only_pub_main_index: Option<usize>,
}

impl<'a> Script<'a> {
    pub fn parse(input: &'a str) -> Result<Self, ParseError> {
        let items = parser::parse(input)?;
        let mut names = HashSet::new();
        let mut only_pub_main_index = None;
        let mut pub_count = 0;

        for (index, item) in items.iter().enumerate() {
            let name = item.fn_signature.name;

            assert!(names.insert(name));

            let is_pub = item.is_pub;

            if is_pub {
                pub_count += 1;
            }

            if is_pub && name == "main" {
                only_pub_main_index = Some(index);
            }
        }

        if pub_count != 1 {
            only_pub_main_index = None;
        }

        Ok(Self {
            items,
            only_pub_main_index,
        })
    }

    pub fn parse_args(
        &self,
        exe_name: &str,
        args: impl IntoIterator<Item = String>,
    ) -> (String, Vec<String>) {
        let app = App::new(exe_name);

        if let Some(main_index) = self.only_pub_main_index {
            self.single_item_args(main_index, app, args)
        } else {
            self.subcmd_args(app, args)
        }
    }

    fn subcmd_args(
        &'a self,
        mut app: App<'a>,
        args: impl IntoIterator<Item = String>,
    ) -> (String, Vec<String>) {
        let mut name_to_args = HashMap::new();
        app = app.setting(AppSettings::SubcommandRequiredElseHelp);

        for item in &self.items {
            let name = item.fn_signature.name;

            if item.is_pub {
                let mut subcmd_app = App::new(name);

                if let Some(desc) = item.description {
                    subcmd_app = subcmd_app.about(desc);
                }
        
                let (subcmd, arg_names) = item_arg_spec(subcmd_app, item);

                name_to_args.insert(name, arg_names);
                app = app.subcommand(subcmd);
            }
        }

        let arg_matches = app.get_matches_from(args);
        let (name, subcmd_matches) = arg_matches.subcommand().unwrap();

        (
            name.to_owned(),
            extract_args(subcmd_matches, name_to_args.remove(name).unwrap()),
        )
    }

    fn single_item_args(
        &'a self,
        main_index: usize,
        app: App<'a>,
        args: impl IntoIterator<Item = String>,
    ) -> (String, Vec<String>) {
        let item = &self.items[main_index];
        let (mut app, arg_names) = item_arg_spec(app, item);

        if let Some(desc) = item.description {
            app = app.about(desc);
        }

        let arg_matches = app.get_matches_from(args);

        (
            item.fn_signature.name.to_owned(),
            extract_args(&arg_matches, arg_names),
        )
    }
}

fn item_arg_spec<'a>(mut app: App<'a>, item: &'a Item) -> (App<'a>, Vec<&'a str>) {
    let mut arg_names = Vec::new();

    for &arg_name in &item.fn_signature.args {
        app = app.arg(Arg::new(arg_name).required(true).multiple_values(false));
        arg_names.push(arg_name);
    }

    (app, arg_names)
}

fn extract_args(arg_matches: &ArgMatches, arg_names: Vec<&str>) -> Vec<String> {
    arg_names
        .into_iter()
        .map(|arg_name| {
            let mut values = arg_matches.values_of(arg_name).unwrap();
            let value = values.next().unwrap();
            assert!(values.next().is_none());

            value.to_owned()
        })
        .collect()
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
    description: Option<&'a str>,
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
