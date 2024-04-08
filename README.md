# Weighted Code Coverage

[![Actions Status][actions badge]][actions]
[![CodeCov][codecov badge]][codecov]
[![Wcc][wcc badge]][wcc]
[![LICENSE][license badge]][license]
[![dependency status][status badge]][status]

Weighted Code Coverage is a tool that implements some metrics aiming to merge information regarding coverage and complexity into a single value.

This repository uses [rust code analysis](https://github.com/mozilla/rust-code-analysis/)
to analyze the project folder and [grcov](https://github.com/mozilla/grcov)
to produce the coverage data used as `weighted-code-coverage` input.

## Algorithms

The implemented algorithms are:
- [Weighted code coverage](https://www.sifis-home.eu/wp-content/uploads/2023/03/D2.4-Final-Developer-Guidelines.pdf) by Luca Ardito and others.
- [CRAP](https://testing.googleblog.com/2011/02/this-code-is-crap.html#:~:text=CRAP%20is%20short%20for%20Change,partner%20in%20crime%20Bob%20Evans) by Alberto Savoia and Bob Evans.
- [SkunkScore](https://www.fastruby.io/blog/code-quality/intruducing-skunk-stink-score-calculator.html) by Ernesto Tagwerker.

First of all, we need to introduce the concept of **code space**, which refers to any structure that incorporates a function, such as another *function*, or a *class*, a *struct*, a *namespace*, or more generally, the *unit scope*. Therefore, the analyzed source code must be divided into code spaces.

### Wcc

Given a code space ***c***, if its complexity is greater than *15*, its weight is equal to the total number of covered lines, otherwise is *0*. To compute the Wcc of the file, one must take all the weights of the code spaces that compose it, sum them up, and then divide the result by the sum of Physical Lines of Code (PLOC) of all the considered code spaces. Wcc is a metric expressed as a percentage and, like coverage, it should be maximized.

### CRAP

Given a code space ***c***, having coverage equal to $cov(c)$, where $cov(c) \in [0, 1]$, and complexity equal to $comp(c)$, the CRAP is computed using the following formula:

$$CRAP(c) = comp(c)^2 \times (1 - cov(c))^3 + comp(c)$$

The CRAP of the file is computed with the same formula, using the average code spaces complexity as $comp$ value and file total coverage as $cov$ value. CRAP is a score and it has to be kept as low as possible.

### Skunk

Given a code space ***c***, having coverage equal to $cov(c)$, where $cov(c) \in [0, 1]$, and complexity equal to $comp(c)$ the CRAP is computed using the following formula:

$$Skunk(f) = \frac{comp(f)}{COMPLEXITY \ FACTOR} \times (1 - cov(f)) + comp(f)$$

$COMPLEXITY \ FACTOR$ is an empirically obtained value, equal to *0.60*.

The Skunk of the file is computed with the same formula, using the average code spaces complexity as $comp$ value and file total coverage as $cov$ value. Skunk is a score and it has to be kept as low as possible.

## Usage

Run `weighted-code-coverage` on a project with the following command:
```
weighted-code-coverage [OPTIONS] --project-path <PROJECT_PATH> (--coveralls <COVERALLS> | --covdir <COVDIR>)
```

Example with some options:
```
weighted-code-coverage --project-path /path/to/project --coveralls /path/to/coveralls.json --thresholds 60.0,16.0,16.0 -t 8 -m files -s wcc --json /path/to/output.json --html /path/to/output/html/ -v
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
weighted-code-coverage --project-path <PROJECT_PATH> --covdir ./covdir.json
```

### Thresholds

To choose the *thresholds* use `--thresholds`.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --coveralls <COVERALLS> --thresholds 60.0,10.0,10.0
```

The default values are *60.0,10.0,10.0*, and the values refer to: **Wcc**, **cyclomatic complexity**, and **cognitive complexity**.

The **Wcc** is a percentage value and an actual threshold, so following the convention commonly used also for coverage, we have assumed a default value of *60%*.

The other two values are complexity values, the first referring to **cyclomatic complexity** while the second referring to **cognitive complexity**. The tool indeed computes two variants for the metric values: one considering *cyclomatic* complexity and the other considering *cognitive* complexity. The maximum value of a code space complexity should normally fall within the range [10, 15], although it is generally recommended to keep it below *10*. A complexity exceeding *15* indicates that the function is complex and may require refactoring.

These two complexity values are used to calculate the effective threshold values for CRAP and Skunk. Assuming a coverage value of 60% we obtain a **CRAP** threshold equal to:

$$CRAP\_{thr} = 10^2 \times (1 - 0.6)^3 + 10 = 16.4$$

and a **Skunk** threshold equal to:

$$Skunk\_{thr} = \frac{10}{0.6} \times (1 - 0.6) + 10 = 16.7$$

The user can thus choose the complexity values that will be used to compute the actual **CRAP** and **Skunk** thresholds, while the coverage value is fixed at *60%*.

### Threads

To choose the number of threads that will be used for the computation use `--threads` or `-t`.

If not specified the default value is the maximum number of threads - 1.

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

The *files* mode will return only the metric values of the files, while the *functions* mode will also show the metric values of the functions.

### Sort

To choose which metric to use for the sorting of the output use `--sort` or `-s`.

The supported values are: *wcc*, *crap*, and *skunk*.
If not specified the default value is *wcc*.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --coveralls <COVERALLS> -s crap
```

### Output

The tool will produce by default a *json* output named *wcc.json* in the current directory. The user can change the path using `--json`.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --coveralls <COVERALLS> --json ./wcc_output.json
```

In addition it is also possible to obtain an *html* output, using the `--html` option and specifying the path of the destination directory.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --coveralls <COVERALLS> --html ./output/html/
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

- At the end, launch `weighted-code-coverage` with the desired options.

## License

Released under the [MIT License](LICENSE).

<!-- Links -->
[actions]: https://github.com/SoftengPoliTo/weighted-code-coverage/actions
[codecov]: https://codecov.io/gh/SoftengPoliTo/weighted-code-coverage
[wcc]: https://softengpolito.github.io/weighted-code-coverage/
[license]: LICENSES/MIT.txt
[status]: https://deps.rs/repo/github/SoftengPoliTo/weighted-code-coverage

<!-- Badges -->
[actions badge]: https://github.com/SoftengPoliTo/weighted-code-coverage/workflows/weighted-code-coverage/badge.svg
[codecov badge]: https://codecov.io/gh/SoftengPoliTo/weighted-code-coverage/branch/master/graph/badge.svg
[wcc badge]: .github/badges/wcc.svg
[license badge]: https://img.shields.io/badge/license-MIT-blue.svg
[status badge]: https://deps.rs/repo/github/SoftengPoliTo/weighted-code-coverage/status.svg
