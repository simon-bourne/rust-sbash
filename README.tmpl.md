# SBash

[![MIT/Apache-2 licensed](https://img.shields.io/crates/l/silkenweb)](./LICENSE-APACHE)

SBash is a Bash based scripting language for workflows. It features better support for functions, and an automatically generated command line interface.

## Installing

```bash
git clone https://github.com/simon-bourne/sbash
cd sbash
cargo install --path .
```

## A Simple Example

This is a really simple script with 2 public functions: `workflow1` and `named-workflow`. We'll use it to look at the command line interface that SBash generates for us, and see how to run functions.

```bash
{{{ include "examples/simple.sb" }}}
```

SBash will generate help text for the command line interface from the doc comments. Here is the main help text for the script:

{{{ shell "to-html examples/simple.sb" }}}

Each function has more detailed documentation, generated from the comments. Here is the help text for the `named-workflow` function:

{{{ shell "to-html 'examples/simple.sb named-workflow --help'" }}}

To run a workflow we run the script with the function name first, followed by it's arguments. For example, to run `named-workflow`:

{{{ shell "to-html 'examples/simple.sb named-workflow my-name'" }}}

## Tutorial

The following serves as a tutorial and illustrates all SBash features:

```bash
{{{ include "examples/all-features.sb" }}}
```

See [workflows.sb](workflows.sb) for another example.
