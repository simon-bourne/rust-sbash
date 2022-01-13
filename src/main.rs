use std::{
    env,
    error::Error,
    fs,
    io::Write,
    iter,
    os::unix::prelude::CommandExt,
    process::{Command, Stdio},
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();
    args.next();
    let script_file = args.next().unwrap();
    let input = fs::read(&script_file)?;

    let mut child = Command::new("bash")
        .arg0(script_file)
        .args(iter::once("-s".to_owned()).chain(args))
        .stdin(Stdio::piped())
        .spawn()?;

    child.stdin.as_mut().unwrap().write_all(&input)?;
    // TODO: Is this OK? Do zombies get cleaned up when we exit?
    let exit_code = child.wait()?;

    println!("Exit code: {}", exit_code);
    Ok(())
}
