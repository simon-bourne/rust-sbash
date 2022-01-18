use std::{fs, io::Write, path::Path};

use goldenfile::Mint;
use sbash::Script;

macro_rules! tests{
    ($($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                test(stringify!($name), "tests/scripts", ".");
            }
        )*
    }
}

tests!(public, inline, hyphen_in_arg);

fn example(name: &str) {
    test(name, "examples", "examples");
}

#[test]
fn simple() {
    example("simple");
}

#[test]
fn all_features() {
    example("all-features");
}

fn test(script: &str, source_dir: &str, target_dir: &str) {
    let mut mint = Mint::new(Path::new("tests/goldenfiles").join(target_dir));
    let mut output = mint
        .new_goldenfile(Path::new(script).with_extension("txt"))
        .unwrap();
    let input =
        fs::read_to_string(Path::new(source_dir).join(script).with_extension("sb")).unwrap();

    match Script::parse(&input) {
        Ok(items) => write!(output, "{}", items),
        Err(e) => write!(output, "{}", e.text()),
    }
    .unwrap();
}
