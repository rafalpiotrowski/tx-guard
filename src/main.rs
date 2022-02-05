use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use tokio::task::JoinHandle;
use tokio::{
    fs::File,
    sync::{
        mpsc::{self, Receiver, Sender},
    },
};
use tokio_stream::StreamExt;

use futures::future::*;

#[macro_use]
extern crate serde;

type ClientId = u16;
type TxId = u32;
type Amount = rust_decimal::Decimal;

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum TxType {
    Deposit,
    Withdrawal,
    Dispiute,
    Resolve,
    Chargeback,
    EOF
}
impl Default for TxType {
    fn default() -> Self { TxType::EOF }
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, Default)]
struct Transaction {
    #[serde(rename(deserialize = "type"))]
    tx_type: TxType,
    #[serde(rename(deserialize = "client"))]
    client_id: ClientId,
    #[serde(rename(deserialize = "tx"))]
    tx_id: TxId,
    #[serde(rename(deserialize = "amount"), with = "rust_decimal::serde::float")]
    // at the moment Amount is not limited to the number of digits after the decimal
    amount: Amount,
}

#[derive(Serialize, Debug, Clone, Copy)]
struct Account {
    #[serde(rename(deserialize = "client"))]
    client_id: ClientId,
    #[serde(rename(deserialize = "available"), with = "rust_decimal::serde::str")]
    available_amount: Amount,
    #[serde(rename(deserialize = "held"), with = "rust_decimal::serde::str")]
    held_amount: Amount,
    #[serde(rename(deserialize = "total"), with = "rust_decimal::serde::str")]
    total_amount: Amount,
    #[serde(rename(deserialize = "locked"))]
    is_locked: bool,
}

#[derive(Debug, Clone)]
struct AccountProcess {
    client_id: ClientId,
    transactions: Sender<Transaction>,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let data_file_path = String::from(&args[1]);

    let (tx_account, mut rx_account) = mpsc::channel::<Account>(32);
    let (tx_transaction, mut rx_transaction) = mpsc::channel::<Transaction>(32);
    
    let processing_data = process_data_file(data_file_path, tx_transaction);
    let process_transactions = process_transactions(rx_transaction);

    let r = tokio::join!(
        processing_data,
        process_transactions
    );

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
                let (tx_transaction, rx_transaction) = mpsc::channel::<Transaction>(32);
                let t1 = t.clone();
                tx_transaction.send(t1).await;
                procs.insert(
                    t.client_id,
                    AccountProcess {
                        client_id: t.client_id,
                        transactions: tx_transaction,
                    },
                );
                let jh = tokio::spawn(async move {
                    process_account_transactions(t.client_id, rx_transaction).await;
                });
            }
            Some((k, proc)) => {
                proc.transactions.send(t).await;
            }
        }
    }

    //println!("finished distributing transactions");

    let last_tx = Transaction::default();
    
    for p in procs.values() {
        p.transactions.send(last_tx).await;
        p.transactions.closed().await;
        //println!("accountprocess {} tx is closed: {}", p.client_id, p.transactions.is_closed());
    }
}

async fn process_data_file(
    data_file_path: String,
    tx: Sender<Transaction>,
) -> std::io::Result<()> {
    let mut rdr = csv_async::AsyncReaderBuilder::new()
        .delimiter(b',')
        .trim(csv_async::Trim::All)
        .has_headers(true)
        .create_deserializer(File::open(data_file_path).await?);

    let mut records = rdr.deserialize::<Transaction>();
    while let Some(record) = records.next().await {
        let record = record?;
        tx.send(record).await;
    }

    //println!("finished processing data");

    Ok(())
}

async fn process_account_transactions(
    id: ClientId,
    mut rx: Receiver<Transaction>
) {
    use TxType::*;

    let mut account = Account {
        client_id: id,
        available_amount: Amount::new(0, 4),
        held_amount: Amount::new(0, 4),
        is_locked: false,
        total_amount: Amount::new(0, 4),
    };

    while let Some(t) = rx.recv().await {
        //println!("processing {:?}", t);
        match t.tx_type {
            Deposit => account.available_amount += t.amount,
            Withdrawal => {}
            Dispiute => {}
            Resolve => {}
            Chargeback => {}
            EOF => {
                break
            }
        }
    }

    println!("{:?}", account);
}
