use serde::Deserialize;
use tokio::sync::mpsc::Sender;

use crate::{
    csv::{RawAccount, RawTransaction},
    ClientId, Money, TxId,
};

#[derive(Debug, Clone)]
pub struct Transaction {
    pub tx_type: TxType,
    pub client_id: ClientId,
    pub tx_id: TxId,
    pub amount: Money,
}
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
                                Ok(value) => value,
                                Err(e) => panic!("cannot convert amount '{}' to f32", str_amount),
                            }
                        }
                    },
                    TxType::Dispute | TxType::Resolve | TxType::Chargeback => 0.0,
                }
            },
            tx_type: t.tx_type,
            tx_id: t.tx_id,
            client_id: t.client_id,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum TxType {
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

#[derive(Debug)]
pub struct Account {
    pub client_id: ClientId,
    // The total funds that are available for trading, staking, withdrawal, etc. This
    // should be equal to the total - held amounts
    pub available_amount: Money,
    // The total funds that are held for dispute. This should be equal to total - available amounts
    pub held_amount: Money,
    // The total funds that are available or held. This should be equal to available + held
    pub total_amount: Money,
    pub is_locked: bool,
}
impl From<Account> for RawAccount {
    fn from(source: Account) -> Self {
        RawAccount {
            client_id: source.client_id,
            available_amount: source.available_amount,
            held_amount: source.held_amount,
            total_amount: source.total_amount,
            is_locked: source.is_locked,
        }
    }
}

#[derive(Debug)]
pub struct AccountProcess {
    pub client_id: ClientId,
    pub transactions: Sender<Option<Transaction>>,
}
