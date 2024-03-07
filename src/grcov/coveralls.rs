use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

use super::get_file_path;

use crate::error::Result;

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct CoverallsSourceFile {
    pub(crate) name: String,
    pub(crate) coverage: Vec<Option<i32>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CoverallsJson {
    source_files: Vec<CoverallsSourceFile>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Coveralls(pub(crate) HashMap<String, CoverallsSourceFile>);

impl Coveralls {
    pub(crate) fn new<A: AsRef<Path>>(json: String, project_path: A) -> Result<Coveralls> {
        let coveralls_json: CoverallsJson = serde_json::from_str(&json)?;
        let mut coveralls = Coveralls(HashMap::new());

        coveralls_json
            .source_files
            .into_iter()
            .try_for_each(|file| -> Result<()> {
                let file_path = get_file_path(&project_path, &file.name)?;
                coveralls.0.insert(file_path, file);
                Ok(())
            })?;

        Ok(coveralls)
    }
}

#[cfg(test)]
mod tests {

    use super::Coveralls;
    use std::{fs, path::Path};

    const COVERALLS_PATH: &str = "./tests/grcov_files/grcov_coveralls.json";

    #[test]
    fn test_coveralls() {
        let json = fs::read_to_string(COVERALLS_PATH).unwrap();
        let coveralls = Coveralls::new(json, Path::new("./project/path/")).unwrap();

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(coveralls, @r###"
          ---
          "./project/path/examples/single_app.rs":
            name: examples/single_app.rs
            coverage:
              - ~
              - 0
          "./project/path/src/app.rs":
            name: src/app.rs
            coverage:
              - ~
              - 5
          "./project/path/src/error.rs":
            name: src/error.rs
            coverage:
              - 25
              - ~
          "###)
        });
    }
}
