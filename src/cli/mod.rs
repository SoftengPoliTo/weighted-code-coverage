use std::error::Error;
use std::path::PathBuf;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Parser;
use tracing_subscriber::EnvFilter;

use weighted_code_coverage::{
    Complexity, JsonFormat, Mode, OutputFormat, Sort, Thresholds, WccRunner,
};

const fn thresholds_long_help() -> &'static str {
    "Set four  thresholds in this order: -t WCC_PLAIN, WCC_QUANTIZED, CRAP, SKUNK\n
    All the values must be floats\n
    All Thresholds has 0 as minimum value, thus no threshold at all.\n
    WCC PLAIN has a max threshold of COMP*SLOC/PLOC\n
    WCC QUANTIZED has a max threshold of 2*SLOC/PLOC\n
    CRAP has a max threshold of COMP^2 +COMP\n
    SKUNK has a max threshold of COMP/25\n"
}

fn select_json_format(coveralls: Option<JsonFormat>, covdir: Option<JsonFormat>) -> JsonFormat {
    match (coveralls, covdir) {
        (Some(coveralls), None) => Some(coveralls),
        (None, Some(covdir)) => Some(covdir),
        _ => None,
    }
    .expect("An option should be set between `coveralls` and `covdir`")
}

// Parse a single key-value pair
fn parse_key_val<T, U>(s: &str) -> Result<(T, U), Box<dyn Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct CargoArgs {
    /// Path to a Cargo.toml
    #[clap(long)]
    pub(crate) manifest_path: Option<PathBuf>,
    #[clap(flatten)]
    pub(crate) args: Args,
}

#[derive(Parser, Debug)]
#[clap(author, version, about)]
pub(crate) struct Args {
    /// Path of the project folder
    #[clap(long = "path_file", value_hint = clap::ValueHint::DirPath)]
    pub(crate) path_file: PathBuf,
    /// Choose complexity metric to use along with thresholds values
    #[arg(short, value_parser = parse_key_val::<Complexity, Thresholds>, long_help = thresholds_long_help())]
    complexity: (Complexity, Thresholds),
    /// Number of threads to use for concurrency
    #[clap(default_value_t = 2)]
    n_threads: usize,
    /// Path of the grcov json file with coveralls format
    #[clap(long, conflicts_with = "covdir", value_parser = JsonFormat::coveralls_parser, value_hint = clap::ValueHint::DirPath)]
    coveralls: Option<JsonFormat>,
    /// Path of the grcov json file with covdir format
    #[clap(long, conflicts_with = "coveralls", value_parser = JsonFormat::covdir_parser, value_hint = clap::ValueHint::DirPath)]
    covdir: Option<JsonFormat>,
    /// Choose mode to use for analysis
    #[clap(long, short = 'm', default_value= Mode::default_value(), value_parser = PossibleValuesParser::new(Mode::all())
        .map(|s| s.parse::<Mode>().unwrap()))]
    mode: Mode,
    /// Sort complex value with the chosen metric
    #[clap(long, short, default_value = Sort::default_value(), value_parser = PossibleValuesParser::new(Sort::all())
        .map(|s| s.parse::<Sort>().unwrap()))]
    sort: Sort,
    /// Output file format
    #[clap(long, short, default_value = OutputFormat::default_value(), value_parser = PossibleValuesParser::new(OutputFormat::all())
        .map(|s| s.parse::<OutputFormat>().unwrap()))]
    output_format: OutputFormat,
    /// Path of the output file
    #[clap(long = "output_path", value_hint = clap::ValueHint::DirPath)]
    output_path: PathBuf,
    #[clap(short, long, global = true)]
    verbose: bool,
}

pub(crate) fn run_weighted_code_coverage(args: Args) {
    // Enable filter to log the information contained in the lib.
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| {
            if args.verbose {
                EnvFilter::try_new("debug")
            } else {
                EnvFilter::try_new("info")
            }
        })
        .unwrap();

    // Run tracer.
    tracing_subscriber::fmt()
        .without_time()
        .with_env_filter(filter_layer)
        .with_writer(std::io::stderr)
        .init();

    WccRunner::new()
        .complexity(args.complexity)
        .n_threads(args.n_threads.max(1))
        .json_format(select_json_format(args.coveralls, args.covdir))
        .mode(args.mode)
        .sort_by(args.sort)
        .output_format(args.output_format)
        .output_path(args.output_path)
        .run(args.path_file)
        .unwrap();
}
