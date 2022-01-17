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
#!/usr/bin/env sbash

#^ A really simple example

#> Workflow 1
#>
#> It doesn't do much!
pub fn workflow1() {
    # The function body is a plain old bash script
    workflow_n 1
}

#> Workflow 2
#>
#> It doesn't do much either!
pub fn named-workflow(
    name #< The name of the workflow
) {
    # SBash sets `$name` for us from the arguments
    workflow "$name"
}

#> This is a private function, and won't appear in the command line interface
fn workflow(name) {
    echo "Workflow $name"
}
```

SBash will generate help text for the command line interface from the doc comments. Here is the main help text for the script:

<pre class="terminal">
<span class="shell">&gt; </span><span class="cmd">examples/simple.sb</span>
<span style='color:#0a0'>sbash</span> 
A really simple example

<span style='color:#a50'>USAGE:</span>
    simple.sb [OPTIONS] [SUBCOMMAND]

<span style='color:#a50'>OPTIONS:
</span>        <span style='color:#0a0'>--debug</span>          Show the generated bash script for a subcommand
    <span style='color:#0a0'>-h</span>, <span style='color:#0a0'>--help</span>           Print help information
        <span style='color:#0a0'>--show-script</span>    Show the generated bash script (without subcommand code)

<span style='color:#a50'>SUBCOMMANDS</span><span style='color:#a50'>:
</span>    <span style='color:#0a0'>help</span>              Print this message or the help of the given subcommand(s)
    <span style='color:#0a0'>named-workflow</span>    Workflow 2
    <span style='color:#0a0'>workflow1</span>         Workflow 1
<span class="shell">&gt; </span><span class="caret"> </span>
</pre>

Each function has more detailed documentation, generated from the comments. Here is the help text for the `named-workflow` function:

<pre class="terminal">
<span class="shell">&gt; </span><span class="cmd">examples/simple.sb</span> <span class="arg">named-workflow</span> <span class="flag">--help</span>
<span style='color:#0a0'>simple.sb-named-workflow</span> 
Workflow 2

It doesn&#39;t do much either!

<span style='color:#a50'>USAGE:</span>
    simple.sb named-workflow &lt;name&gt;

<span style='color:#a50'>ARGS:
</span>    <span style='color:#0a0'>&lt;name&gt;</span>
            The name of the workflow

<span style='color:#a50'>OPTIONS:
</span>    <span style='color:#0a0'>-h</span>, <span style='color:#0a0'>--help</span>
            Print help information
<span class="shell">&gt; </span><span class="caret"> </span>
</pre>

To run a workflow we run the script with the function name first, followed by it's arguments. For example, to run `named-workflow`:

<pre class="terminal">
<span class="shell">&gt; </span><span class="cmd">examples/simple.sb</span> <span class="arg">named-workflow</span> <span class="arg">my-name</span>
Workflow my-name
<span class="shell">&gt; </span><span class="caret"> </span>
</pre>

## Tutorial

The following serves as a tutorial and illustrates all SBash features:

```bash
#!/usr/bin/env sbash

#^ This is the short description for the whole script
#^
#^ The long description will include this entire block of text. Try running
#^ `examples/all-features.sb --help`
#^
#^ An empty doc comment line will start a new paragraph. Doc comments come in
#^ 3 flavours:
#^
#^ `#^` attaches the doc comment to the parent. This is only valid for the
#^ overall script documentation.
#^
#^ The other 2 types of doc comments are valid for functions and arguments:
#^
#^ `#>` attaches to the item following.
#^
#^ `#<` attaches to the item preceding.

#> This is a public function.
#>
#> A public function will appear a command line option.
#> Try `examples/all-features.sb a-public-function --help`
pub fn a-public-function() {
    # Functions are compiled to normal bash functions, so we can call them 
    # from other functions:
    echo "Hello from a-public-function"
    echo "Calling a-private-function"
    a-private-function
}

#> A function can have arguments
#>
#> Try `examples/all-features.sb a-function-with-arguments --help` to see
#> documentation for the arguments 
pub fn a-function-with-arguments(
    first_arg,
        #< This is how you document arguments. This is documentation for
        #< `first-arg`
    #> Or you can document them like this if you prefer. This is documentation
    #> for `second-arg`.
    second_arg
) {
    echo Hello from a-function-with-arguments
    # Arguments are passed to the function as environment variables
    echo First arg: "$first_arg"
    echo Second arg: "$second_arg"
}

#> A function can forward any extra arguments using `$@` as it's last argument
#>
#> Try `examples/all-features.sb forwarding-arguments 1 2 3 4 5`
pub fn forwarding-arguments(first_arg, second_arg, $@) {
    echo First arg: "$first_arg"
    echo Second arg: "$second_arg"
    echo Remaining arguments:

    for arg in "$@"; do
        echo "$arg"
    done
}

#> This is a private function.
#>
#> It won't appear as a command line option.
fn a-private-function() {
    # The body of a function is just a bash script
    echo "Hello from a-private-function"
}

#> Normally, each function runs in it's own subshell, so any
#> variable/directory changes don't escape the function scope. `inline`
#> functions don't run in a subshell.
inline fn an-inline-function() {
    my-variable="This variable will be set in the calling function"
    echo Hello from an-inline-function
}

pub inline fn public-functions-can-be-inline-too() {
    echo Hello from public-functions-can-be-inline-too
}
```

See [workflows.sb](workflows.sb) for another example.
