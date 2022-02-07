# txp
simple transaction verification client

output of the application is send to the std out

run `cargo run --help` to get possible usage information:

Running `target\debug\txp-cli.exe --help`
```
txp 0.1.0
Rafal Piotrowski <rafalpiotrowski@users.noreply.github.com>
Transaction Processing System

USAGE:
    txp-cli.exe [OPTIONS] <file>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -b, --buffer <buffer>      Size of the channel buffer [default: 32]
    -t, --tracing <tracing>    Tracing level [possible values: Error, Warn, Info, Debug, Trace]

ARGS:
    <file>    CSV file to process
```

## Tests
in the project root folder type `cargo test`

## Data file correctnes
At the moment, if supplied data file has any errors (e.g. missing column, wrong formatting etc) system will exit with panic! giving details about the problem.

## input data file format: 
```
type, client, tx, amount

type: String
client: u16 (max 65_535)
tx: u32 (max 4_294_967_295)
amount: f32 decimal value with precision of upto 4 places past the decimal (system will accept input with any precision)
```

## External Dependencies
`futures = "0.3.21"` (https://crates.io/crates/futures)

`tokio = { version = "1.16.1", features = ["full"] }` (https://crates.io/crates/tokio)

`tokio-stream = "0.1.8"` (https://crates.io/crates/tokio-stream)

`tracing = "0.1.30"` (https://crates.io/crates/tracing)

`tracing-subscriber = "0.3.8"` (https://crates.io/crates/tracing-subscriber)

`structopt = "0.3.26"` (https://crates.io/crates/structopt)

`serde = { version = "1.0.136", features = ["derive"] }` (https://crates.io/crates/serde)

`csv-async = { version = "1.2.4", features = ["with_serde", "tokio"] }` (https://crates.io/crates/csv-async)

## Security vulnerabilities
run `cargo audit` (https://lib.rs/crates/cargo-audit) to get report on the possible security issues

# Architecture

## Cargo project
Solution is split into 2 parts:
1. library composed of the following files:
    - src/lib.rs
    - src/csv.rs
    - src/tx.rs
2. bin (executable) cli client located in:
    - bin/cli.rs

## Library Modules
### 1. csv
In this module we have all functionality related to parsing CSV input data.
Function `CsvTransactionReader::process_data_file` in `src/csv.rs` is the future that is executed asynchronously using tokio runtime

### 2. tx
In this module we have all functionality related to processing input transactions and spawning seperate tasks that handle transactions for given account. 
We spawn 1 task per client account, that is responsible for processing it's transactions. (see implementation of `TxProcessor` in `src/tx.rs')

`TxProcessor::process_transactions` is a future that is run asynchronously together with `CsvTransactionReader::process_data_file`

## How to handle multiple data sources?
TxProcessor
