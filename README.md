# txp
simple transaction verification client

output of the application is send to the std out

run 'cargo run --help' to get possible usage information:

Running `target\debug\txp-cli.exe --help`

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


# Data file correctnes
At the moment, if supplied data file has any errors (e.g. missing column, wrong formatting etc) system will exit with panic! giving details about the problem.

## input data file format: 

type, client, tx, amount

tx type: String

client id: u16 (max 65535)

tx id: u32 (max 4294967295)

tx amount: decimal value with precision of upto 4 places past the decimal (system will accept input with any precision)

# Stream values through memory

# How to handle multiple data sources?


# External Dependencies
`futures = "0.3.21"` (https://crates.io/crates/futures)

`tokio = { version = "1.16.1", features = ["full"] }` (https://crates.io/crates/tokio)

`tokio-stream = "0.1.8"` (https://crates.io/crates/tokio-stream)

`tracing = "0.1.30"` (https://crates.io/crates/tracing)

`tracing-subscriber = "0.3.8"` (https://crates.io/crates/tracing-subscriber)

`structopt = "0.3.26"` (https://crates.io/crates/structopt)

`serde = { version = "1.0.136", features = ["derive"] }` (https://crates.io/crates/serde)

`csv-async = { version = "1.2.4", features = ["with_serde", "tokio"] }` (https://crates.io/crates/csv-async)
