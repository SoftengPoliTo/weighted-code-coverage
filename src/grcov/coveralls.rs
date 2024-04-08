use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::error::Result;

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct CoverallsSourceFile {
    pub(crate) name: PathBuf,
    pub(crate) coverage: Vec<Option<i32>>,
}

#[derive(Debug, Deserialize)]
struct CoverallsJson {
    source_files: Vec<CoverallsSourceFile>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Coveralls(pub(crate) HashMap<PathBuf, CoverallsSourceFile>);

impl Coveralls {
    pub(crate) fn new(json_path: &Path, project_path: &Path) -> Result<Coveralls> {
        let json = fs::read_to_string(json_path)?;
        let coveralls_json: CoverallsJson = serde_json::from_str(&json)?;
        let mut coveralls = Coveralls(HashMap::new());

        coveralls_json.source_files.into_iter().for_each(|file| {
            coveralls
                .0
                .insert(project_path.to_path_buf().join(&file.name), file);
        });

        Ok(coveralls)
    }
}

#[cfg(test)]
mod tests {

    use super::Coveralls;
    use std::path::Path;

    const COVERALLS_PATH: &str = "./tests/grcov_files/grcov_coveralls.json";

    #[test]
    fn test_coveralls() {
        let coveralls =
            Coveralls::new(Path::new(COVERALLS_PATH), Path::new("project/test/path/")).unwrap();

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(coveralls, @r###"
            ---
            project/test/path/examples/single_app.rs:
              name: examples/single_app.rs
              coverage:
                - ~
                - 0
            project/test/path/src/app.rs:
              name: src/app.rs
              coverage:
                - ~
                - 5
            project/test/path/src/error.rs:
              name: src/error.rs
              coverage:
                - 25
                - ~
            "###)
        });
    }
}
