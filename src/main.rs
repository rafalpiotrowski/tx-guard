use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use tokio::task::JoinHandle;
use tokio::{
    fs::File,
    sync::mpsc::{self, Receiver, Sender},
};
use tokio_stream::StreamExt;

use futures::future::*;

use tracing::{trace, info};
use txp::{
    csv::RawTransaction,
    tx::{Account, AccountProcess, Transaction},
    ClientId, Money, Result, TxId,
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
    tracing_subscriber::fmt::try_init()?;

    let args: Vec<String> = std::env::args().collect();
    let data_file_path = String::from(&args[1]);

    let (tx_account, mut rx_account) = mpsc::channel::<Account>(32);
    let (tx_transaction, mut rx_transaction) = mpsc::channel::<Option<Transaction>>(32);

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
        txp::csv::CsvTransactionReader::process_data_file(data_file_path, process_raw_transaction);

    let process_transactions = process_transactions(rx_transaction);

    let r = tokio::join!(data_reader, process_transactions);

    Ok(())
}

async fn process_transactions(mut rx: Receiver<Option<Transaction>>) {
    let mut procs = HashMap::<ClientId, AccountProcess>::new();

    let mut tx_count = 0;

    while let Some(Some(t)) = rx.recv().await {
        tx_count += 1;
        //println!("processing tx {tx_count} {:?}", t);
        let link = procs.get_key_value(&t.client_id);
        match link {
            None => {
                let (tx_transaction, rx_transaction) = mpsc::channel::<Option<Transaction>>(32);
                procs.insert(
                    t.client_id,
                    AccountProcess {
                        client_id: t.client_id,
                        transactions: tx_transaction.clone(),
                    },
                );
                let jh = tokio::spawn(async move {
                    process_account_transactions(t.client_id, rx_transaction).await;
                });
                tx_transaction.send(Some(t)).await;
            }
            Some((k, proc)) => {
                proc.transactions.send(Some(t)).await;
            }
        }
    }

    info!("finished distributing transactions: shutting down account tasks");

    for p in procs.values() {
        p.transactions.send(Option::None).await;
        p.transactions.closed().await;
        trace!("accountprocess {} tx is closed: {}", p.client_id, p.transactions.is_closed());
    }

    info!("done")
}

async fn process_account_transactions(id: ClientId, mut rx: Receiver<Option<Transaction>>) {
    use txp::tx::TxType::*;

    let mut account = Account {
        client_id: id,
        available_amount: 0.0,
        held_amount: 0.0,
        is_locked: false,
        total_amount: 0.0,
    };

    while let Some(Some(t)) = rx.recv().await {
        //println!("processing {:?}", t);
        match t.tx_type {
            Deposit => account.available_amount += t.amount,
            Withdrawal => {}
            Dispute => {}
            Resolve => {}
            Chargeback => {
                account.is_locked = true;
            }
        }
    }

    account.total_amount = account.available_amount + account.held_amount;

    //println!("{:?}", account);
}
