use hbcn::{CLIArguments, constrain_main, analyse_main, depth_main};
use clap::Parser;
use anyhow::Result;

fn main() -> Result<()> {
    let args = CLIArguments::parse();

    match args {
        CLIArguments::Constrain(args) => constrain_main(args),
        CLIArguments::Analyse(args) => analyse_main(args),
        CLIArguments::Depth(args) => depth_main(args),
    }
}
