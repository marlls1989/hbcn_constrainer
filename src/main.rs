use anyhow::Result;
use clap::Parser;
use hbcn::{CLIArguments, analyse_main, constrain_main, expand_main};

fn main() -> Result<()> {
    let args = CLIArguments::parse();

    // Initialize global verbose flag
    hbcn::output_suppression::set_verbose(args.verbose);

    match args.command {
        hbcn::CLICommand::Expand(args) => expand_main(args),
        hbcn::CLICommand::Analyse(args) => analyse_main(args),
        hbcn::CLICommand::Constrain(args) => constrain_main(args),
    }
}
