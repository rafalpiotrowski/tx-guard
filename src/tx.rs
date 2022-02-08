use std::collections::HashMap;

use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::{debug, trace, warn};

use crate::{account::Account, csv::RawTransaction, ClientId, Transaction, TxId, TxType};

/// convert RawTransaction into Transaction
impl From<RawTransaction> for Transaction {
    fn from(t: RawTransaction) -> Self {
        Transaction {
            amount: {
                match t.tx_type {
                    TxType::Deposit | TxType::Withdrawal => match t.amount {
                        None => 0.0,
                        Some(str_amount) => {
                            let r = str_amount.parse::<f32>();
                            match r {
                                Ok(value) => {
                                    if value >= 0.0 {
                                        value
                                    } else {
                                        panic!("amount '{}' < 0.0", value)
                                    }
                                }
                                Err(_e) => panic!("cannot convert amount '{}' to f32", str_amount),
                            }
                        }
                    },
                    TxType::Dispute | TxType::Resolve | TxType::Chargeback => 0.0,
                }
            },
            tx_type: t.tx_type,
            tx_id: t.tx_id,
            client_id: t.client_id,
            in_dispute: false,
        }
    }
}

/// simple data storage for account process to store client id and tx_sender
#[derive(Debug)]
pub struct AccountProcess {
    pub client_id: ClientId,
    pub tx_sender: Sender<Option<Transaction>>,
}

/// Transaction processing functionality
pub struct TxProcessor {}

impl TxProcessor {
    /// Transaction processing task
    ///
    /// `tx_receiver` channel for receiving incomming transactions to process
    /// `buffer_size` size of the buffer used when spawning each new account tx task
    pub async fn process_transactions(
        mut tx_receiver: Receiver<Option<Transaction>>,
        buffer_size: usize,
    ) {
        // map client/account to AccountProcess
        let mut account_processes = HashMap::<ClientId, AccountProcess>::new();

        while let Some(Some(t)) = tx_receiver.recv().await {
            trace!("processing tx {:?}", t);
            let account_process = account_processes.get_key_value(&t.client_id);
            match account_process {
                //
                None => {
                    let (acc_tx_sender, acc_tx_receiver) =
                        mpsc::channel::<Option<Transaction>>(buffer_size);
                    account_processes.insert(
                        t.client_id,
                        AccountProcess {
                            client_id: t.client_id,
                            tx_sender: acc_tx_sender.clone(),
                        },
                    );
                    //create new task to handle
                    tokio::spawn(async move {
                        TxProcessor::process_account_transactions(t.client_id, acc_tx_receiver)
                            .await;
                    });
                    // todo: handle the Result
                    let _ = acc_tx_sender.send(Some(t)).await;
                }
                Some((_k, proc)) => {
                    // todo: handle the Result
                    let _ = proc.tx_sender.send(Some(t)).await;
                }
            }
        }

        debug!("finished distributing transactions: shutting down account tasks");

        // no more transaction to process, inform our account tasks to stop listening and print the account status
        for p in account_processes.values() {
            let _ = p.tx_sender.send(Option::None).await;
            p.tx_sender.closed().await;
            trace!(
                "accountprocess {} tx is closed: {}",
                p.client_id,
                p.tx_sender.is_closed()
            );
        }

        debug!("all account processing tasks has been closed");
    }

    /// this function is spawn for each client account to handle its transactions
    ///
    /// `id` client id
    /// `mut tx_reveiver` receiver part of the channel to listen for incomming transactions to process.
    ///     If None is received its a signal to print the account status and exit
    async fn process_account_transactions(
        id: ClientId,
        mut tx_reveiver: Receiver<Option<Transaction>>,
    ) {
        let mut account = Account::default();
        account.client_id = id;

        debug!("created account {:?}", &account);

        //local history of transactions made on this account
        let mut transactions = HashMap::<TxId, Transaction>::new();

        // wait for incomming transactions, if None received we exit the loop
        while let Some(Some(t)) = tx_reveiver.recv().await {
            trace!("account {} processing {:?}", account.client_id, t);
            let r = account.process_transaction(&t, &mut transactions);
            match r {
                Ok(a) => account = a,
                Err(e) => {
                    warn!("{:?}", e);
                }
            }
            // store only Deposit and Withdrawal transactions for possible dispute/resolve/chargeback events
            // for simplicity we assume that we receive only once given transaction
            if t.tx_type == TxType::Deposit || t.tx_type == TxType::Withdrawal {
                transactions.insert(t.tx_id, t);
            }

            trace!("account state: {:?}", &account);
        }

        debug!("exiting; final account state {:?}", account);

        // print account data to stdout
        println!(
            "{},{:.4},{:.4},{:.4},{}",
            account.client_id,
            account.available_amount,
            account.held_amount,
            account.total_amount,
            account.is_locked
        );
    }
}
