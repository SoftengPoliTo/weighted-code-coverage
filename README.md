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
- [Skunk](https://www.fastruby.io/blog/code-quality/intruducing-skunk-stink-score-calculator.html) by Ernesto Tagwerker.

First of all, we need to introduce the concept of **code space**, which refers to any structure that incorporates a function, such as another *function*, a *class*, a *struct*, a *namespace*, or more generally, the *unit scope*. Therefore, the analyzed source code must be divided into code spaces.

### Wcc

Let ***c*** be a code space, if its complexity is greater than *15*, its weight is equal to the total number of covered lines, otherwise is *0*. To compute the Wcc of a file, one must take all the weights of the code spaces that compose it, sum them up, and then divide the result by the sum of Physical Lines of Code (PLOC) of all the considered code spaces. Wcc is a metric expressed as a percentage and, like coverage, it should be maximized.

### CRAP

Let ***c*** be a code space with coverage equal to $cov(c)$, where $cov(c) \in [0, 1]$, and complexity equal to $comp(c)$, the CRAP is computed using the following formula:

$$CRAP(c) = comp(c)^2 \times (1 - cov(c))^3 + comp(c)$$

The CRAP of a file is computed with the same formula, using the average code spaces complexity as $comp$ value and file total coverage as $cov$ value. CRAP is a score and it has to be kept as low as possible.

### Skunk

Let ***c*** be a code space with coverage equal to $cov(c)$, where $cov(c) \in [0, 1]$, and complexity equal to $comp(c)$. The Skunk of ***c*** is computed using the following formula:

$$
Skunk(c) = \frac{comp(c)}{COMPLEXITY \ FACTOR} \times (100 - cov(c)) + comp(c)
$$

where $COMPLEXITY \ FACTOR$ is an empirically obtained value, equal to *60*.

The Skunk of a file is computed with the same formula, using the average code spaces complexity as $comp$ value and file total coverage as $cov$ value. Skunk is a score and it has to be kept as low as possible.

To obtain the previous formula, we started from the [introduction](https://www.fastruby.io/blog/code-quality/intruducing-skunk-stink-score-calculator.html) to the metric, whitin which the presented algorithm can be summarized by the following formula:

$$
Skunk(c) =
\begin{cases}
\frac{comp(c)}{COMPLEXITY \ FACTOR}, & \text{if }cov(c) = 100 \\
\frac{comp(c)}{COMPLEXITY \ FACTOR} \times (100 - cov(c)), & \text{otherwise}
\end{cases}
$$

where $COMPLEXITY \ FACTOR$ is equal to *25*.

Analyzing it, we noticed that it presents some issues. First of all, the fact that if we consider a code space with complexity equal to *50* and maximum coverage of *100%*, we get a Skunk equal to *2*:

$$
Skunk(c) = \frac{comp(c)}{COMPLEXITY \ FACTOR} = \frac{50}{25} = 2
$$

This value is anomalous, as despite the coverage being maximum, it is not reasonable for such a complex function to have such a low Skunk score.

To mitigate this issue, we have modified the formula as follows:

$$
Skunk(c) = \frac{comp(c)}{COMPLEXITY \ FACTOR} \times (100 - cov(c)) + comp(c)
$$

Another problem with this metric is that when we try to compute the [threshold](#thresholds) value, assuming coverage equal to *60%* and complexity equal to *10*, we notice that using a $COMPLEXITY \ FACTOR$ equal to *25* would result in a threshold value of *26*:

$$
Skunk(c) = \frac{10}{25} \times (100 - 60) + 10 = 26
$$

which causes even code spaces with complexities up to *26* to be accepted by the metric, in fact a code space with maximum coverage of *100%* and complexity equal to *26* would have a Skunk equal to *26*:

$$
Skunk(c) = \frac{26}{25} \times (100 - 100) + 26 = 26
$$

Instead, defining a $COMPLEXITY \ FACTOR$ equal to *60* yields a threshold value of *16.7*:

$$Skunk_{thr} = \frac{10}{0.6} \times (1 - 0.6) + 10 = 16.7$$

Therefore, based on these considerations, we have decided to adopt the initially presented formula with a $COMPLEXITY \ FACTOR$ of 60.
However, note that **Skunk**, among the three, is the most problematic and least accurate metric. In fact, the way the initial $COMPLEXITY \ FACTOR$ equal to *25* is obtained is not adequately documented by the author, and as stated in this [video](https://www.youtube.com/watch?v=ZyU6K6eR-_A&t=1492s) that introduces the metric, it is defined as a sort of magic number.

## Usage

Run `weighted-code-coverage` on a project with the following command:
```
weighted-code-coverage [OPTIONS] --project-path <PROJECT_PATH> --grcov-format <GRCOV_FORMAT> --grcov-path <GRCOV_PATH> 
```

Example with some options:
```
weighted-code-coverage --project-path /path/to/project --grcov-format coveralls --grcov-path /path/to/coveralls.json --thresholds 60.0,16.0,16.0 -t 8 -m files -s wcc --json /path/to/output.json --html /path/to/output/html/ -v
```

### Grcov Format and Path

To specify the input grcov json file you must first select the format using `--grcov-format` option followed by *coveralls* or *covdir*, and then specify the file path using `--grcov-path` option.

*coveralls* example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --grcov-format coveralls --grcov-path ./coveralls.json
```

*covdir* example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --grcov-format covdir --grcov-path ./covdir.json
```

### Thresholds

To choose the *thresholds* use `--thresholds` option.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --grcov-format <GRCOV_FORMAT> --grcov-path <GRCOV_PATH> --thresholds 60.0,10.0,10.0
```

The default values are *60.0,10.0,10.0*, and the values refer to: **Wcc**, **cyclomatic complexity**, and **cognitive complexity**.

The **Wcc** is a percentage value and an actual threshold, so following the convention commonly used also for coverage, we have assumed a default value of *60%*.

The other two are complexity values, the first one refers to **cyclomatic complexity**, while the second one refers to **cognitive complexity**. The tool computes two variants for metric values: one considering *cyclomatic* complexity and the other considering *cognitive* complexity. The maximum value of a code space complexity should normally fall within the range [10, 15], although it is generally recommended to keep it below *10*. A complexity exceeding *15* indicates a complex function which may require refactoring.

These two complexity values are used to calculate the threshold for CRAP and Skunk. Assuming a coverage value of 60% we obtain a **CRAP** threshold equal to:

$$CRAP_{thr} = 10^2 \times (1 - 0.6)^3 + 10 = 16.4$$

and a **Skunk** threshold equal to:

$$Skunk_{thr} = \frac{10}{0.6} \times (1 - 0.6) + 10 = 16.7$$

A user can choose which complexity values will be used to compute **Crap** and **Skunk** thresholds, while the coverage value is fixed at *60%*.

### Threads

To choose the number of threads that will be used for the computation use `--threads` or `-t` option.

If not specified, the default value is:

$$
threads_{max} - 1
$$

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --grcov-format <GRCOV_FORMAT> --grcov-path <GRCOV_PATH> -t 7
```

### Mode

To choose the mode to use for analysis.
use `--mode` or `-m` option.

The supported values are *files* and *functions*.
If not specified the default value is *files*.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --grcov-format <GRCOV_FORMAT> --grcov-path <GRCOV_PATH> -m functions
```

The *files* mode will return only the metric values of the files, while the *functions* mode will also show the metric values of the functions.

### Sort

To choose which metric to use for the sorting of the output use `--sort` or `-s` option.

The supported values are: *wcc*, *crap*, and *skunk*.
If not specified, the default value is *wcc*.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --grcov-format <GRCOV_FORMAT> --grcov-path <GRCOV_PATH> -s crap
```

### Output

The tool will produce by default a *json* output named *wcc.json* in the current directory. The user can change the path using `--json` option.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --grcov-format <GRCOV_FORMAT> --grcov-path <GRCOV_PATH> --json ./wcc_output.json
```

In addition, it is also possible to obtain an *html* output, using the `--html` option and specifying the path of the destination directory.

Example:
```
weighted-code-coverage --project-path <PROJECT_PATH> --grcov-format <GRCOV_FORMAT> --grcov-path <GRCOV_PATH> --html ./output/html/
```

## Steps to install and run weighted-code-coverage

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
