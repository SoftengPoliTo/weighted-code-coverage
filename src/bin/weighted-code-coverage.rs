#[path = "../cli/mod.rs"]
mod cli;

use clap::Parser;

use cli::{run_weighted_code_coverage, Args};

fn main() {
    let args = Args::parse();
    run_weighted_code_coverage(args);
}
