use anyhow::Result;
use clap::Parser;
use hbcn::{CLIArguments, analyse_main, constrain_main, expand_main};

fn main() -> Result<()> {
    let args = CLIArguments::parse();

    match args {
        CLIArguments::Expand(args) => expand_main(args),
        CLIArguments::Analyse(args) => analyse_main(args),
        CLIArguments::Constrain(args) => constrain_main(args),
    }
}
