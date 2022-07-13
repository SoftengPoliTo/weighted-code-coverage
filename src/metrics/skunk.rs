use rust_code_analysis::FuncSpace;
use serde_json::Value;

use crate::error::*;
use crate::utility::{get_coverage_perc, get_covered_lines, Complexity};

const COMPLEXITY_FACTOR: f64 = 25.0;
// Calculate the Skunkscore value  for the given file
// https://www.fastruby.io/blog/code-quality/intruducing-skunk-stink-score-calculator.html
// In this implementation the code smells are ignored.
// Return the value in case of success and an specif error in case of fails
pub(crate) fn skunk_nosmells(
    root: &FuncSpace,
    covs: &[Value],
    metric: Complexity,
    coverage: Option<f64>,
) -> Result<f64> {
    let comp = match metric {
        Complexity::Cyclomatic => root.metrics.cyclomatic.cyclomatic_sum(),
        Complexity::Cognitive => root.metrics.cognitive.cognitive_sum(),
    };
    let cov = if let Some(coverage) = coverage {
        coverage
    } else {
        get_coverage_perc(covs)? * 100.
    };
    Ok(if cov == 100. {
        comp / COMPLEXITY_FACTOR
    } else {
        (comp / COMPLEXITY_FACTOR) * (100. - (cov))
    })
}

// Calculate the Skunkscore value for a function
// https://www.fastruby.io/blog/code-quality/intruducing-skunk-stink-score-calculator.html
// In this implementation the code smells are ignored.
// Return the value in case of success and an specif error in case of fails
pub(crate) fn skunk_nosmells_function(
    space: &FuncSpace,
    covs: &[Value],
    metric: Complexity,
    coverage: Option<f64>,
) -> Result<f64> {
    let comp = match metric {
        Complexity::Cyclomatic => space.metrics.cyclomatic.cyclomatic_sum(),
        Complexity::Cognitive => space.metrics.cognitive.cognitive_sum(),
    };
    let cov = if let Some(coverage) = coverage {
        coverage / 100.0
    } else {
        let (covered_lines, tot_lines) = get_covered_lines(covs, space.start_line, space.end_line)?;
        if tot_lines != 0. {
            covered_lines / tot_lines
        } else {
            0.0
        }
    };
    Ok(if cov == 100. {
        comp / COMPLEXITY_FACTOR
    } else {
        (comp / COMPLEXITY_FACTOR) * (100. - (100. * cov))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utility::{get_root, read_json};
    use std::fs;

    const JSON: &str = "./data/data.json";
    const PREFIX: &str = "../rust-data-structures-main/";
    const SIMPLE: &str = "../rust-data-structures-main/data/simple_main.rs";
    const FILE: &str = "./data/simple_main.rs";
    const COMP: Complexity = Complexity::Cyclomatic;
    const COGN: Complexity = Complexity::Cognitive;

    #[test]
    fn test_skunk_cyclomatic() {
        let file = fs::read_to_string(JSON).unwrap();
        let covs = read_json(file, PREFIX).unwrap();
        let root = get_root(FILE).unwrap();
        let vec = covs.get(SIMPLE).unwrap().to_vec();
        let skunk = skunk_nosmells(&root, &vec, COMP, None).unwrap();
        assert_eq!(skunk, 6.4);
    }

    #[test]
    fn test_skunk_cognitive() {
        let file = fs::read_to_string(JSON).unwrap();
        let covs = read_json(file, PREFIX).unwrap();
        let root = get_root(FILE).unwrap();
        let vec = covs.get(SIMPLE).unwrap().to_vec();
        let skunk_cogn = skunk_nosmells(&root, &vec, COGN, None).unwrap();
        assert_eq!(skunk_cogn, 4.8);
    }

    #[test]
    fn test_skunk_cyclomatic_function() {
        let file = fs::read_to_string(JSON).unwrap();
        let covs = read_json(file, PREFIX).unwrap();
        let root = get_root(FILE).unwrap();
        let vec = covs.get(SIMPLE).unwrap().to_vec();
        let skunk = skunk_nosmells_function(&root, &vec, COMP, None).unwrap();
        assert_eq!(skunk, 6.4);
    }

    #[test]
    fn test_skunk_cognitive_function() {
        let file = fs::read_to_string(JSON).unwrap();
        let covs = read_json(file, PREFIX).unwrap();
        let root = get_root(FILE).unwrap();
        let vec = covs.get(SIMPLE).unwrap().to_vec();
        let skunk_cogn = skunk_nosmells_function(&root, &vec, COGN, None).unwrap();
        assert_eq!(skunk_cogn, 4.8);
    }
}
