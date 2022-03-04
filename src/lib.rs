pub mod crap;
pub mod sifis;
pub mod skunk;
pub mod utility;

use crate::crap::crap;
use crate::sifis::{sifis_plain, sifis_quantized};
use crate::skunk::skunk_nosmells;
use crate::utility::*;
use std::fs;
use std::path::*;
/// This Function get the folder of the repo to analyzed and the path to the json obtained using grcov
/// It prints all the SIFIS, CRAP and SkunkScore values for all the Rust files in the folders
/// the output will be print as follows:
/// FILE       | SIFIS PLAIN | SIFIS QUANTIZED | CRAP       | SKUNKSCORE
pub fn get_metrics<A: AsRef<Path> + Copy, B: AsRef<Path> + Copy>(
    files_path: A,
    json_path: B,
) -> Result<(), SifisError> {
    let vec = match read_files(files_path.as_ref()) {
        Ok(vec) => vec,
        Err(_err) => {
            return Err(SifisError::WrongFile(
                files_path.as_ref().display().to_string(),
            ))
        }
    };

    let file = match fs::read_to_string(json_path) {
        Ok(file) => file,
        Err(_err) => {
            return Err(SifisError::WrongFile(
                json_path.as_ref().display().to_string(),
            ))
        }
    };
    let covs = read_json(file, files_path.as_ref().to_str().unwrap())?;
    //println!("FILE \t SIFIS PLAIN \t SIFIS QUANTIZED \t CRAP \t SKUNKSCORE");
    println!(
        "{0: <20} | {1: <20} | {2: <20} | {3: <20} | {4: <20}",
        "FILE", "SIFIS PLAIN", "SIFIS QUANTIZED", "CRAP", "SKUNKSCORE"
    );
    for path in vec {
        let arr = match covs.get(&path) {
            Some(arr) => arr.to_vec(),
            None => return Err(SifisError::HashMapError(path)),
        };
        let p = Path::new(&path);
        let sifis = sifis_plain(p, &arr)?;
        let sifis_quantized = sifis_quantized(p, &arr)?;
        let crap = crap(p, &arr)?;
        let skunk = skunk_nosmells(p, &arr)?;
        println!(
            "{0: <20} | {1: <20.3} | {2: <20.3} | {3: <20.3} | {4: <20.3}",
            p.file_name().unwrap().to_str().unwrap(),
            sifis,
            sifis_quantized,
            crap,
            skunk
        );
    }
    Ok(())
}