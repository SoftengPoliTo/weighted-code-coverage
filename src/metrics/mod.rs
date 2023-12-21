pub mod crap;
pub mod skunk;
pub mod wcc;

use std::path::Path;

use rust_code_analysis::{get_function_spaces, guess_language, read_file, FuncSpace};
use tracing::debug;

use crate::error::{Error, Result};

// Get the code coverage in percentage between start and end
pub(crate) fn get_covered_lines(
    covs: &[Option<i32>],
    start: usize,
    end: usize,
) -> Result<(f64, f64)> {
    // Count the number of covered lines
    let (tot_lines, covered_lines) =
        covs.iter()
            .enumerate()
            .try_fold((0., 0.), |acc, (i, line)| -> Result<(f64, f64)> {
                let is_none = line.is_none();
                let sum;
                if !is_none && (start - 1..end).contains(&i) {
                    // let cov = line.as_u64().ok_or(Error::ConversionError())?;
                    // This unwrap can't panic because we have previously checked that `line` is not none
                    let cov = line.unwrap();
                    if cov > 0 {
                        sum = (acc.0 + 1., acc.1 + 1.);
                    } else {
                        sum = (acc.0 + 1., acc.1);
                    }
                } else {
                    sum = (acc.0, acc.1);
                }
                Ok(sum)
            })?;
    Ok((covered_lines, tot_lines))
}

// Get the root FuncSpace from a file
pub(crate) fn get_root<A: AsRef<Path>>(path: A) -> Result<FuncSpace> {
    let data = read_file(path.as_ref())?;
    let lang = guess_language(&data, path.as_ref())
        .0
        .ok_or(Error::Language)?;
    debug!("{:?} is written in {:?}", path.as_ref(), lang);
    let root = get_function_spaces(&lang, data, path.as_ref(), None).ok_or(Error::Metrics)?;
    Ok(root)
}
