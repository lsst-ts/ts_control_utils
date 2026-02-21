# Control Utilities

This is the control utilities used in the control system in Rust.

## Code Format

To format the code, do:

```bash
.githooks/pre-commit
```

## Unit Test

Each module and function have the related unit tests.
Since the CI test is needed, you can use the [cargo-nextest](https://crates.io/crates/cargo-nextest) instead of the built-in test framework.
Do the following to run all tests:

```bash
cargo nextest run
```

To test a single module, do:

```bash
cargo nextest run --lib $module_name
```

To generate the `junit.xml` (ouput path is `target/nextest/ci/junit.xml`), do:

```bash
cargo nextest run --profile ci
```

## Version History

See [here](doc/version_history.md) for the version history.
