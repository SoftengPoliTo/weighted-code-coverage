# Weighted Code Coverage

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
weighted-code-coverage [OPTIONS] --path_file <PATH_FILE> --path_json <PATH_JSON>
```
Example with some options:

```
weighted-code-coverage  --path_file /path/to/source/code --path_json /path/to/coveralls.json -c cyclomatic --json /path/to/output.json -f coveralls -m files -t 35.0,1.5,35.0,30.0
```

### Complexity
To choose complexity metric to use.
use the *complexity* `c` option.

It supports only these values: *cyclomatic*, *cognitive*.
If not specified the default value is *cyclomatic*.

Example:
```
weighted-code-coverage --path_file <PATH_FILE> --path_json <PATH_JSON> -c cognitive
```

### JSON Format
To specify the json format used for the json file.
use the *json-format* `f` option.

It supports only these values: *coveralls*, *covdir*.
If not specified the default value is *coveralls*.

Example:
```
weighted-code-coverage --path_file <PATH_FILE> --path_json <PATH_JSON> -f coveralls
```

### Mode
To choose the mode to use for analysis.
use the *mode* `m` option.

It supports only these values: *files*, *functions*.
If not specified the default value is *files*.

Example:
```
weighted-code-coverage --path_file <PATH_FILE> --path_json <PATH_JSON> -m functions
```

### Thresholds
To set four thresholds for evaluation during the analysis.
use the *thresholds* `t` option. 

A string must be given with all the 4 algorithms separated by comma *,* in this specific order:
*WCC PLAIN*,*WCC QUANTIZED*,*CRAP*,*SKUNK*

If not specified the default value are 35.0,1.5,35.0,30.0.

Example:
```
weighted-code-coverage --path_file <PATH_FILE> --path_json <PATH_JSON> -t 50.0,0.7,65.0,45.0
```

### Threads
To choose the number of thread to launch for the application.
Use the *n_threads* `n` option. 

If not specified the default value is 2.
Can launch at minimum 2 threads.

Example:
```
weighted-code-coverage --path_file <PATH_FILE> --path_json <PATH_JSON> -n 16
```

### Sort By
To choose which metric to use for sorting the complex files/functions.
Use the *sort_by* `s` option. 

It supports only these values: *wcc_plain*, *wcc_quantized*, *crap*, *skunk*.
If not specified the default value is *wcc_plain*.

Example:
```
weighted-code-coverage --path_file <PATH_FILE> --path_json <PATH_JSON> -s crap
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

Distributed under the terms of the MIT license - See LICENSE for details.
