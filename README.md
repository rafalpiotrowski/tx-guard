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
Unit tests are only for `Account` in `account.rs` since this is the main business logic
Integration tests are in folder `tests/` together with some test files

## Data file correctnes
At the moment, if supplied data file has any errors (e.g. missing column, wrong formatting etc) system will exit with panic! giving details about the problem.

## input data file format: 
Program accepts only valid data, and panic otherwise on first occourance of any inccorect data 
```
type, client, tx, amount

type: String
client: u16 (max 65_535)
tx: u32 (max 4_294_967_295)
amount: f32 decimal value with precision of upto 4 places past the decimal (system will accept input with any precision) and >= 0.0
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
At the moment audit did not identify any security issues.
run `cargo audit` (https://lib.rs/crates/cargo-audit) to get report on the possible security issues

# Architecture

Solution is based on clasical producer/consumer model. We start with 2 tasks
1. CsvTransactionReader::process_data_file, acting here as producer.
2. TxProcessor::process_transactions, acting here as consumer
During the operation of TxProcessor more tasks are created 1 for each Account (i.e. client_id). Like wise here TxProcessor::process_transactions acts like producer for each TxProcessor::process_account_transactions task.

## Memory usage

Since we can have max 65_535 accounts and 4_294_967_295 transactions in the total max memory usage whould be around 70GB :
- Transaction size is 12 bytes (total max size in memory 69 GB)
- Account size is 16 bytes (total max size in memory 1 MB)
- plus memory used to store list of taks etc.

Even if we would have 80% of Deposit and Withraw transactions, it still takes more then we can store in RAM.

For this we would need to use some sort of database to store transactions for lookup and not to keep them in running memory.

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

`TxProcessor::process_transactions` is a future that runs asynchronously together with `CsvTransactionReader::process_data_file`. Program waits for them to both finish before exiting.

## How to handle multiple data sources?
In order to support multiple datasources we would need to implement producer like the one in CsvTransactionReader::process_data_file so it would act as another producer. It's fairly straight forward as are already using MultiProducer/SingleConsumer channels.

With multi data sources, we could no longer use Option<RawTransaction>. Dedicated message would need to be created to identify the source, necessary for the system to know how many producers there are, so the consumer `TxProcessor::process_transactions` could handle shutdown properly.