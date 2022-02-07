use tokio::{
    sync::mpsc::{self},
};

use tracing::{info, trace, Level};
use tracing_subscriber::FmtSubscriber;
use txp::{
    csv::{CsvTransactionReader, RawTransaction},
    tx::{Transaction, TxProcessor},
    Result,
};

/// Entry point for CLI tool.
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
    // Enable logging
    // tracing_subscriber::fmt::try_init()?;

    // a builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::ERROR)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let args: Vec<String> = std::env::args().collect();
    let data_file_path = String::from(&args[1]);

    let (tx_transaction, mut rx_transaction) = mpsc::channel::<Option<Transaction>>(32);

    // function clousure that converts raw transaction into transaction and sends it down for processing 
    // when we get None to process, it is the signal to finish processing
    let process_raw_transaction = |t: Option<RawTransaction>| async {
        let send_result = match t {
            Some(rt) => tx_transaction.send(Some(rt.into())).await,
            None => tx_transaction.send(Option::None).await,
        };
        match send_result {
            Ok(_) => Ok(()),
            Err(e) => Err("Failed to send transaction down the channel".to_string()),
        }
    };

    let data_reader =
        CsvTransactionReader::process_data_file(data_file_path, process_raw_transaction);

    let process_transactions = TxProcessor::process_transactions(rx_transaction);

    println!("client,available,held,total,locked");

    let r = tokio::join!(data_reader, process_transactions);

    Ok(())
}