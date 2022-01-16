use std::{fs, io::Write, path::Path};

use goldenfile::Mint;
use sbash::Script;

macro_rules! tests{
    ($($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                test(stringify!($name));
            }
        )*
    }
}

tests!(public, inline, hyphen_in_arg);

fn test(script: &str) {
    let mut mint = Mint::new("tests/goldenfiles");
    let mut output = mint
        .new_goldenfile(Path::new(script).with_extension("txt"))
        .unwrap();
    let input =
        fs::read_to_string(Path::new("tests/scripts").join(script).with_extension("sb")).unwrap();

    match Script::parse(&input) {
        Ok(items) => write!(output, "{}", items),
        Err(e) => write!(output, "{}", e.text()),
    }
    .unwrap();
}
