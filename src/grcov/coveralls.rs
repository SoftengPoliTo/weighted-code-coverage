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

#[derive(Debug)]
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
