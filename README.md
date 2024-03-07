# Weighted Code Coverage

[![Actions Status][actions badge]][actions]
[![CodeCov][codecov badge]][codecov]
[![LICENSE][license badge]][license]
[![dependency status][status badge]][status]

This repository contains the implementations of some Weighted Code Coverage algorithms
for some of the languages supported by rust-code-analysis.

This repository uses [rust code analysis](https://github.com/mozilla/rust-code-analysis/)
to analyze a project folder and [grcov](https://github.com/mozilla/grcov)
to produce the coverage data used as `weighted-code-coverage` input.

## Algorithms

The implemented algorithms are:
- WCC by Luca Ardito and others (https://www.sifis-home.eu/wp-content/uploads/2021/10/D2.2_SIFIS-Home_v1.0-to-submit.pdf (section 2.4.1))
- CRAP by Alberto Savoia and Bob Evans(https://testing.googleblog.com/2011/02/this-code-is-crap.html#:~:text=CRAP%20is%20short%20for%20Change,partner%20in%20crime%20Bob%20Evans. )
- SkunkScore by Ernesto Tagwerker (https://www.fastruby.io/blog/code-quality/intruducing-skunk-stink-score-calculator.html )

### WCC
Two version available for this algorithm:
- WCC PLAIN
- WCC QUANTIZED

WCC PLAIN give each line of the code a complexity value of the file/function , then we sum all the covered lines and divide the result by the PLOC of the file/function.

WCC QUANTIZED we analyze each line of the file if the line is not covered then we dive a weight of 0 , else if the complexity of the block(usually function) the line is part of is greater than 15 we assign a weight of 2 otherwise 1. We sum all the weight and then divide the result by the PLOC of the file

### CRAP
Take the total complexity of the file and the coverage in percentage then apply the following formula formula: 
```(comp^2)*(1-coverage) +comp```
The higher the result the more complex is the file.
### SKUNK
Take the total complexity of the file , the coverage in percentage, and a COMPLEXITY_FACTOR in this case equal to 25 then apply the following formula formula: 
```(comp/COMPLEXITY_FACTOR)*(100-coverage*100)```
The higher the result the more complex is the file.

## Usage

Run `weighted-code-coverage` on a project with the following command:
```
weighted-code-coverage [OPTIONS] --project-path <PROJECT_PATH> (--coveralls <COVERALLS> | --covdir <COVDIR>)
```

Example with some options:
```
weighted-code-coverage --project-path /path/to/source/code --coveralls /path/to/coveralls.json -c cyclomatic:35.0,1.5,35.0,30.0 -t 8 -m files -s wcc_plain --output-format json --output-path /path/to/output.json -v
```

### Grcov Format
To specify the format of the grcov json file use `--coveralls` or `--covdir` followed by the file path.

Note that the two options are mutual exclusive, but is mandatory to insert one of them.

Example `--coveralls`:
```
weighted-code-coverage --project-path <PROJECT_PATH> --coveralls ./coveralls.json
```

Example `--covdir`:
```
weighted-code-coverage --project_path <PROJECT_PATH> --covdir ./covdir.json
```

### Complexity
To choose *complexity* metric and *thresholds* use `--complexity` or `-c`.

The supported values are *cyclomatic* and *cognitive*.
If not specified the default value for the complexity metric *cyclomatic* while for the tresholds *35.0,1.5,35.0,30.0*.

Tresholds values refer to the thresholds for the 4 algorithms: *WCC PLAIN*, *WCC QUANTIZED*, *CRAP* and *SKUNK*.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --coveralls <COVERALLS> -c cognitive:45.0,2.5,45.0,40.0
```

### Threads
To choose the number of threads that will be used for the computation use `--threads` or `-t`.

If not specified the default value is maximum number of threads - 1.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --coveralls <COVERALLS> -t 7
```

### Mode
To choose the mode to use for analysis.
use `--mode` or `-m`.

The supported values are *files* and *functions*.
If not specified the default value is *files*.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --coveralls <COVERALLS> -m functions
```

### Sort
To choose which metric to use for sorting the complex files/functions use `--sort` or `-s`.

The supported values are: *wcc_plain*, *wcc_quantized*, *crap* and *skunk*.
If not specified the default value is *wcc_plain*.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --coveralls <COVERALLS> -s crap
```

### Output format
To choose the output file format use `--output-format`.

The supported values are *json* and *html*.
If not specified the default value is *json*.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --coveralls <COVERALLS> --outuput-format json
```

### Output path
To choose the path of the output file use `--output-path`.

If not specified the default value is *./wcc_output.json*.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --coveralls <COVERALLS> --outuput-path ./output.json
```

## Steps to install and run weighted-code-coverage

- grcov needs a rust nightly version in order to work, so switch to it with: ``rustup default nightly``
- Install grcov latest version using cargo ``cargo install grcov``
- After grcov has been installed, install `llvm-tools component`:

```
rustup component add llvm-tools-preview
```

- `RUSTFLAGS` and `LLVM_PROFILE_FILE` environment variables need to be set in this way

```
export RUSTFLAGS="-Cinstrument-coverage"
export LLVM_PROFILE_FILE="your_name-%p-%m.profraw"
```

- Then go to the folder of the repository you need to analyze and run all tests with ``cargo test``
- After each test has been passed, some `.profraw` files are generated. To print out the json file with all the coverage information inside, run the following command:

```
grcov . --binary-path ./target/debug/ -t coveralls -s . --token YOUR_COVERALLS_TOKEN > coveralls.json
```

- At the end, launch `weighted-code-coverage` with your desired options

## License

Released under the [MIT License](LICENSE).

<!-- Links -->
[actions]: https://github.com/SoftengPoliTo/weighted-code-coverage/actions
[codecov]: https://codecov.io/gh/SoftengPoliTo/weighted-code-coverage
[license]: LICENSES/MIT.txt
[status]: https://deps.rs/repo/github/SoftengPoliTo/weighted-code-coverage

<!-- Badges -->
[actions badge]: https://github.com/SoftengPoliTo/weighted-code-coverage/workflows/weighted-code-coverage/badge.svg
[codecov badge]: https://codecov.io/gh/SoftengPoliTo/weighted-code-coverage/branch/master/graph/badge.svg
[license badge]: https://img.shields.io/badge/license-MIT-blue.svg
[status badge]: https://deps.rs/repo/github/SoftengPoliTo/weighted-code-coverage/status.svg
