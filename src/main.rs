use std::{
    env,
    error::Error,
    fs,
    io::Write,
    iter,
    process::{self, Command, Stdio},
};

use sbash::{Action, Script};

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();
    let exe_name = args.next().ok_or("Expected ARGV[0]")?;
    let script_file = args
        .next()
        .ok_or(format!("Usage: {} [SCRIPT_FILE]", exe_name))?;
    let input = fs::read_to_string(&script_file)?;
    let items = Script::parse(&input)?;
    let action = items.parse_args(&exe_name, env::args().skip(1));

    let (fn_name, args) = match action {
        Action::FnCall { name, args } => (name, args),
        Action::ShowScript => {
            println!("{}", items);
            return Ok(());
        }
    };

    let script = format!(
        "{}\n\nset -euo pipefail\nBASH_ARGV0=\"{}\"\n\n{} \"$@\"",
        items, script_file, fn_name
    );

    // TODO: Can we make a temporary file for the script so bash can read stdin?
    let mut child = Command::new("bash")
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
