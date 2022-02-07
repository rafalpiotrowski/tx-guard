#![deny(warnings)]

use std::path::PathBuf;

use tokio::{
    sync::mpsc::{self},
};

use tracing::{Level};
use tracing_subscriber::FmtSubscriber;
use txp::{
    csv::{CsvTransactionReader, RawTransaction},
    tx::{Transaction, TxProcessor},
    Result,
};

use structopt::{StructOpt, clap::arg_enum};

arg_enum! {
    #[derive(Debug)]
    enum TracingLevel {
        Error,
        Warn,
        Info,
        Debug,
        Trace
    }
}

#[derive(StructOpt, Debug)]
#[structopt(name = "txp", version = env!("CARGO_PKG_VERSION"), author = env!("CARGO_PKG_AUTHORS"), about = "Transaction Processing System")]
struct Opt {
    /// Tracing level.
    #[structopt(long, short, possible_values = &TracingLevel::variants(), case_insensitive = true)]
    tracing: Option<TracingLevel>,

    /// Size of the channel buffer
    #[structopt(short, long, default_value="32")]
    buffer: usize,

    /// CSV file to process
    #[structopt(name = "file", parse(from_os_str))]
    csv_file: PathBuf,
}

/// Entry point.
///
/// The `[tokio::main]` annotation signals that the Tokio runtime should be
/// started when the function is called. The body of the function is executed
/// within the newly spawned runtime.
///
/// `flavor = "current_thread"` is used here to avoid spawning background
/// threads. The CLI tool use case benefits more by being lighter instead of
/// multi-threaded. use: #[tokio::main(flavor = "current_thread")]
#[tokio::main]
async fn main() -> Result<()> {

    let opt = Opt::from_args();

    let tracing_level = match opt.tracing {
        Some(l) => {
            match l {
                TracingLevel::Error => Level::ERROR,
                TracingLevel::Warn => Level::WARN,
                TracingLevel::Info => Level::INFO,
                TracingLevel::Debug => Level::DEBUG,
                TracingLevel::Trace => Level::TRACE,
            }
        }
        None => Level::ERROR
    };

    // a builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(tracing_level)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let (tx_transaction, rx_transaction) = mpsc::channel::<Option<Transaction>>(opt.buffer);

    // function clousure that converts raw transaction into transaction and sends it down for processing 
    // when we get None to process, it is the signal to finish processing
    let process_raw_transaction = |t: Option<RawTransaction>| async {
        let send_result = match t {
            Some(rt) => tx_transaction.send(Some(rt.into())).await,
            None => tx_transaction.send(Option::None).await,
        };
        match send_result {
            Ok(_) => Ok(()),
            Err(_e) => Err("Failed to send transaction down the channel".to_string()),
        }
    };

    let data_reader =
        CsvTransactionReader::process_data_file(opt.csv_file, process_raw_transaction);

    let process_transactions = TxProcessor::process_transactions(rx_transaction, opt.buffer);

    println!("client,available,held,total,locked");

    tokio::join!(data_reader, process_transactions);

    Ok(())
}