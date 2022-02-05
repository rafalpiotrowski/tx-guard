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

#[macro_use]
extern crate serde;
use serde::{Deserialize, Serialize};

/// Error returned by most functions.
///
/// When writing a real application, one might want to consider a specialized
/// error handling crate or defining an error type as an `enum` of causes.
/// However, for our example, using a boxed `std::error::Error` is sufficient.
///
/// For performance reasons, boxing is avoided in any hot path. For example, in
/// `parse`, a custom error `enum` is defined. This is because the error is hit
/// and handled during normal execution when a partial frame is received on a
/// socket. `std::error::Error` is implemented for `parse::Error` which allows
/// it to be converted to `Box<dyn std::error::Error>`.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// A specialized `Result` type for mini-redis operations.
///
/// This is defined as a convenience.
pub type Result<T> = std::result::Result<T, Error>;

type ClientId = u16;
type TxId = u32;
type Money = f32;

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
enum TxType {
    // A deposit is a credit to the client's asset account, meaning it should increase the available and
    // total funds of the client account
    Deposit,

    // A withdraw is a debit to the client's asset account, meaning it should decrease the available and
    // total funds of the client account
    // If a client does not have sufficient available funds the withdrawal should fail and the total amount
    // of funds should not change
    Withdrawal,

    // A dispute represents a client's claim that a transaction was erroneous and should be reversed.
    // The transaction shouldn't be reversed yet but the associated funds should be held. This means
    // that the clients available funds should decrease by the amount disputed, their held funds should
    // increase by the amount disputed, while their total funds should remain the same.
    // Notice that a dispute does not state the amount disputed. Instead a dispute references the
    // transaction that is disputed by ID. If the tx specified by the dispute doesn't exist you can ignore it
    // and assume this is an error on our partners side.
    Dispute,

    // A resolve represents a resolution to a dispute, releasing the associated held funds. Funds that
    // were previously disputed are no longer disputed. This means that the clients held funds should
    // decrease by the amount no longer disputed, their available funds should increase by the
    // amount no longer disputed, and their total funds should remain the same.
    // Like disputes, resolves do not specify an amount. Instead they refer to a transaction that was
    // under dispute by ID. If the tx specified doesn't exist, or the tx isn't under dispute, you can ignore
    // the resolve and assume this is an error on our partner's side.
    Resolve,

    // A chargeback is the final state of a dispute and represents the client reversing a transaction.
    // Funds that were held have now been withdrawn. This means that the clients held funds and
    // total funds should decrease by the amount previously disputed. If a chargeback occurs the
    // client's account should be immediately frozen.
    // Like a dispute and a resolve a chargeback refers to the transaction by ID (tx) and does not
    // specify an amount. Like a resolve, if the tx specified doesn't exist, or the tx isn't under dispute,
    // you can ignore chargeback and assume this is an error on our partner's side.
    Chargeback,
}

#[derive(Deserialize, Serialize, Debug)]
struct RawTransaction {
    #[serde(rename(deserialize = "type"))]
    tx_type: TxType,
    #[serde(rename(deserialize = "client"))]
    client_id: ClientId,
    #[serde(rename(deserialize = "tx"))]
    tx_id: TxId,

    // this at the moment does not work with csv_async library
    // parser raises error when no value is supplied
    //#[serde(rename(deserialize = "amount"), with = "rust_decimal::serde::float")]
    // amount: Money,
    #[serde(rename(deserialize = "amount"))]
    // work around to handle transactions types where amount is not specified
    amount: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct Transaction {
    tx_type: TxType,
    client_id: ClientId,
    tx_id: TxId,
    amount: Money,
}

/// convert RawTransaction into Transaction
impl From<RawTransaction> for Transaction {
    fn from(t: RawTransaction) -> Self {
        Transaction {
            amount: {
                match t.tx_type {
                    TxType::Deposit | TxType::Withdrawal => {
                        match t.amount {
                            None => 0.0,
                            Some(str_amount) => {
                                let r = str_amount.parse::<f32>();
                                match r {
                                    Ok(value) => value,
                                    Err(e) => panic!("cannot convert amount '{}' to f32", str_amount),
                                }
                            }
                        }        
                    }
                    TxType::Dispute | TxType::Resolve | TxType::Chargeback => 0.0
                }
            },
            tx_type: t.tx_type,
            tx_id: t.tx_id,
            client_id: t.client_id,
        }
    }
}

#[derive(Serialize, Debug)]
struct Account {
    #[serde(rename(deserialize = "client"))]
    client_id: ClientId,

    #[serde(rename(deserialize = "available"))]
    // The total funds that are available for trading, staking, withdrawal, etc. This
    // should be equal to the total - held amounts
    available_amount: Money,

    //#[serde(rename(deserialize = "held"), with = "rust_decimal::serde::str")]
    #[serde(rename(deserialize = "held"))]
    // The total funds that are held for dispute. This should be equal to total - available amounts
    held_amount: Money,

    #[serde(rename(deserialize = "total"))]
    // The total funds that are available or held. This should be equal to available + held
    total_amount: Money,

    #[serde(rename(deserialize = "locked"))]
    is_locked: bool,
}

#[derive(Debug)]
struct AccountProcess {
    client_id: ClientId,
    transactions: Sender<TxProcessingMsg>,
}

#[derive(Debug)]
enum TxProcessingMsg {
    Tx(Transaction),
    Finish
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let data_file_path = String::from(&args[1]);

    let (tx_account, mut rx_account) = mpsc::channel::<Account>(32);
    let (tx_transaction, mut rx_transaction) = mpsc::channel::<Transaction>(32);

    let processing_data = process_data_file(data_file_path, tx_transaction);
    let process_transactions = process_transactions(rx_transaction);

    let r = tokio::join!(processing_data, process_transactions);

    Ok(())
}

async fn process_transactions(mut rx: Receiver<Transaction>) {
    let mut procs = HashMap::<ClientId, AccountProcess>::new();

    let mut tx_count = 0;

    while let Some(t) = rx.recv().await {
        tx_count += 1;
        //println!("processing tx {tx_count} {:?}", t);
        let link = procs.get_key_value(&t.client_id);
        match link {
            None => {
                let (tx_transaction, rx_transaction) = mpsc::channel::<TxProcessingMsg>(32);
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
                tx_transaction.send(TxProcessingMsg::Tx(t)).await;                
            }
            Some((k, proc)) => {
                proc.transactions.send(TxProcessingMsg::Tx(t)).await;
            }
        }
    }

    //println!("finished distributing transactions");

    for p in procs.values() {
        p.transactions.send(TxProcessingMsg::Finish).await;
        p.transactions.closed().await;
        //println!("accountprocess {} tx is closed: {}", p.client_id, p.transactions.is_closed());
    }
}

async fn process_data_file(data_file_path: String, tx: Sender<Transaction>) -> Result<()> {
    let mut rdr = csv_async::AsyncReaderBuilder::new()
        .delimiter(b',')
        .flexible(true)
        .trim(csv_async::Trim::All)
        .has_headers(true)
        .create_deserializer(File::open(data_file_path).await?);

    let mut records = rdr.deserialize::<RawTransaction>();
    while let Some(record) = records.next().await {
        match record {
            Ok(t) => {
                let x = tx.send(t.into()).await;
            }
            Err(err) => {
                println!("error reading CSV file: {}", err);
                std::process::exit(1);
            }
        }
    }

    //println!("finished processing data");

    Ok(())
}

async fn process_account_transactions(id: ClientId, mut rx: Receiver<TxProcessingMsg>) {
    use TxType::*;

    let mut account = Account {
        client_id: id,
        available_amount: 0.0,
        held_amount: 0.0,
        is_locked: false,
        total_amount: 0.0,
    };

    while let Some(t) = rx.recv().await {
        println!("processing {:?}", t);
        match t {
            TxProcessingMsg::Tx(tx) => {
                match tx.tx_type {
                    Deposit => account.available_amount += tx.amount,
                    Withdrawal => {}
                    Dispute => {}
                    Resolve => {}
                    Chargeback => {
                        account.is_locked = true;
                    }
                }
        
            },
            TxProcessingMsg::Finish => break
        }
    }

    account.total_amount = account.available_amount + account.held_amount;

    println!("{:?}", account);
}
