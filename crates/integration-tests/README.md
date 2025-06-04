# Integration tests for twine

## Prerequisities:
1. Update the [config](./res/config.yaml)


## Run tests:

1. Run specific test module

    ```sh
    RUST_LOG=info cargo test --package twine-integration-tests --lib -- deposit::deposit_test --show-output
    ```
