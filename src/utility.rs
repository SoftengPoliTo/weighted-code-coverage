use serde_json::Value;
use std::collections::*;
use std::fs;
use std::path::*;
use thiserror::Error;
/// Customized error messages using thiserror library
#[derive(Error, Debug)]
pub enum SifisError {
    #[error("Error while reading File: {0}")]
    WrongFile(String),
    #[error("Error while converting JSON value to a type")]
    ConversionError(),
    #[error("Error while taking value from HashMap with key : {0}")]
    HashMapError(String),
    #[error("Failing reading JSON from string")]
    ReadingJSONError(),
    #[error("Error while computing Metrics")]
    MetricsError(),
    #[error("Error while guessing language")]
    LanguageError(),
}

///This function read all  the files in the project folder
/// Returns all the Rust files, ignoring the other files or an error in case of problems
pub fn read_files(files_path: &Path) -> Result<Vec<String>, SifisError> {
    let mut vec = vec![];
    let mut first = PathBuf::new();
    first.push(files_path);
    let mut stack = vec![first];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            let paths = match fs::read_dir(path.clone()) {
                Ok(paths) => paths,
                Err(_err) => return Err(SifisError::WrongFile(path.display().to_string())),
            };

            for p in paths {
                stack.push(p.unwrap().path());
            }
        } else {
            let ext = path.extension();

            if ext != None && ext.unwrap() == "rs" {
                vec.push(path.display().to_string());
            }
        }
    }
    Ok(vec)
}

/// This fuction read the content of the coveralls  json file obtain by using grcov
/// Return a HashMap with all the files arrays of covered lines using the path to the file as key
pub fn read_json(file: String, prefix: &str) -> Result<HashMap<String, Vec<Value>>, SifisError> {
    let val: Value = match serde_json::from_str(file.as_str()) {
        Ok(val) => val,
        Err(_err) => return Err(SifisError::ReadingJSONError()),
    };
    let vec = match val["source_files"].as_array() {
        Some(vec) => vec,
        None => return Err(SifisError::ReadingJSONError()),
    };
    let mut covs = HashMap::<String, Vec<Value>>::new();
    for x in vec {
        let mut name = prefix.to_string();
        name += x["name"].as_str().unwrap();
        let value = match x["coverage"].as_array() {
            Some(value) => value.to_vec(),
            None => return Err(SifisError::ConversionError()),
        };
        covs.insert(name.to_string(), value);
    }
    Ok(covs)
}

// Get the code coverage in percentage
pub fn get_coverage_perc(covs: &[Value]) -> Result<f64, SifisError> {
    let mut tot_lines = 0.;
    let mut covered_lines = 0.;
    // count the number of covered lines
    for i in 0..covs.len() {
        let is_null = match covs.get(i) {
            Some(val) => val.is_null(),
            None => return Err(SifisError::ConversionError()),
        };

        if !is_null {
            tot_lines += 1.;
            let cov = match covs.get(i).unwrap().as_u64() {
                Some(cov) => cov,
                None => return Err(SifisError::ConversionError()),
            };
            if cov > 0 {
                covered_lines += 1.;
            }
        }
    }
    Ok(covered_lines / tot_lines)
}

#[cfg(test)]
mod tests {

    use super::*;
    const JSON: &str = "./data/data.json";
    const PREFIX: &str = "../rust-data-structures-main/";
    const MAIN: &str = "../rust-data-structures-main/data/main.rs";
    const SIMPLE: &str = "../rust-data-structures-main/data/simple_main.rs";

    #[test]
    fn test_read_json() {
        let file = fs::read_to_string(JSON).unwrap();
        let covs = read_json(file, PREFIX).unwrap();
        assert_eq!(covs.contains_key(SIMPLE), true);
        assert_eq!(covs.contains_key(MAIN), true);
        let vec = covs.get(SIMPLE).unwrap();
        assert_eq!(vec.len(), 12);
        let vec_main = covs.get(MAIN).unwrap();
        assert_eq!(vec_main.len(), 9);
        let value = vec.get(6).unwrap();
        assert_eq!(value, 2);
        let value_null = vec.get(1).unwrap();
        assert_eq!(value_null.is_null(), true);
    }
}