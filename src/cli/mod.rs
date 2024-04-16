use std::path::PathBuf;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Parser;
use tracing_subscriber::EnvFilter;

use weighted_code_coverage::{GrcovFile, GrcovFormat, Mode, Sort, Thresholds, WccRunner};

#[inline]
fn thresholds_long_help() -> &'static str {
    "The three threshold values are parsed in this order: wcc, cyclomatic complexity, cognitive complexity.
The input string must therefore follow the same order: `-t '60.0,10.0,10.0'`.
Wcc is a percentage value, so its value should be in the [0, 100] range.
The complexities should tipically be in the [0, 15] range,
assuming that a code space with a complexity higher than 15 is too complex."
}

const JSON_OUTPUT_PATH: &str = "./wcc.json";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct CargoArgs {
    /// Path to a Cargo.toml.
    #[clap(long)]
    pub(crate) manifest_path: Option<PathBuf>,
    #[clap(flatten)]
    pub(crate) args: Args,
}

#[derive(Parser, Debug)]
#[clap(author, version, about)]
pub(crate) struct Args {
    /// Path of the project folder.
    #[clap(long, required = true, value_hint = clap::ValueHint::DirPath)]
    pub(crate) project_path: PathBuf,
    /// Format of the grcov json file.
    #[clap(long, required = true, value_parser = PossibleValuesParser::new(GrcovFormat::all())
        .map(|s| s.parse::<GrcovFormat>().unwrap()))]
    grcov_format: GrcovFormat,
    /// Path of the grcov json file.
    #[clap(long, required = true, value_hint = clap::ValueHint::FilePath)]
    grcov_path: PathBuf,
    /// Choose complexity metric to use along with thresholds values.
    #[clap(long, default_value_t = Thresholds::default(), long_help = thresholds_long_help())]
    thresholds: Thresholds,
    /// Number of threads to use for concurrency.
    #[clap(long, short = 't', default_value_t = (rayon::current_num_threads() - 1).max(1))]
    threads: usize,
    /// Choose mode to use for analysis.
    #[clap(long, short = 'm', default_value_t = Mode::Files, value_parser = PossibleValuesParser::new(Mode::all())
        .map(|s| s.parse::<Mode>().unwrap()))]
    mode: Mode,
    /// Sort output files and functions with the chosen metric.
    #[clap(long, short = 's', default_value_t = Sort::Wcc, value_parser = PossibleValuesParser::new(Sort::all())
        .map(|s| s.parse::<Sort>().unwrap()))]
    sort: Sort,
    /// Path of the json output.
    #[clap(long, default_value = JSON_OUTPUT_PATH, value_hint = clap::ValueHint::FilePath)]
    json: PathBuf,
    /// Path of the html output.
    #[clap(long, value_hint = clap::ValueHint::DirPath)]
    html: Option<PathBuf>,
    #[clap(long, short = 'v', global = true)]
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

    // Initialize WccRunner.
    let mut wcc_runner = WccRunner::new()
        .thresholds(args.thresholds)
        .n_threads(args.threads)
        .mode(args.mode)
        .sort_by(args.sort);

    // If present, set the path of the html output directory.
    if let Some(html_path) = &args.html {
        wcc_runner = wcc_runner.html_path(html_path);
    }

    // Define the grcov file.
    let grcov_file = match args.grcov_format {
        GrcovFormat::Coveralls => GrcovFile::Coveralls(args.grcov_path),
        GrcovFormat::Covdir => GrcovFile::Covdir(args.grcov_path),
    };

    // Run WccRunner.
    wcc_runner
        .run(&args.project_path, grcov_file, &args.json)
        .unwrap();
}
