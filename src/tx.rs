use std::collections::HashMap;

use serde::Deserialize;
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{debug, info, trace};

use crate::{
    csv::{RawAccount, RawTransaction},
    ClientId, Money, TxId,
};

#[derive(Debug)]
pub enum AccountError {
    /// Account is frozen, cannot perform any other operation on it
    Frozen(ClientId),

    InssuficientFundsForWithdrawal(ClientId),

    NoTxForDispute(TxId),

    TxNotInDispute(TxId),

    /// other error case
    Other(crate::Error),
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub tx_type: TxType,
    pub client_id: ClientId,
    pub tx_id: TxId,
    pub amount: Money,
    pub in_dispute: bool,
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

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, PartialEq)]
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
impl Default for Account {
    fn default() -> Self {
        Self {
            client_id: Default::default(),
            available_amount: Default::default(),
            held_amount: Default::default(),
            total_amount: Default::default(),
            is_locked: Default::default(),
        }
    }
}
impl Account {
    fn process_transaction(
        &self,
        t: &Transaction,
        history: &mut HashMap<TxId, Transaction>,
    ) -> core::result::Result<Self, AccountError> {
        use TxType::*;

        match t.tx_type {
            Deposit => self.deposit(t.amount),
            Withdrawal => self.withdrawal(t.amount),
            Dispute => self.dispute(t.tx_id, history),
            Resolve => self.resolve(t.tx_id, history),
            Chargeback => self.chargeback(t.tx_id, history),
        }
    }

    fn deposit(&self, amount: Money) -> core::result::Result<Self, AccountError> {
        if self.is_locked {
            Err(AccountError::Frozen(self.client_id))
        } else {
            let mut a = Account::default();
            a.client_id = self.client_id;
            a.available_amount = self.available_amount + amount;
            a.held_amount = self.held_amount;
            a.total_amount = a.available_amount + a.held_amount;
            Ok(a)
        }
    }

    fn withdrawal(&self, amount: Money) -> core::result::Result<Self, AccountError> {
        if self.is_locked {
            Err(AccountError::Frozen(self.client_id))
        } else if self.available_amount < amount {
            Err(AccountError::InssuficientFundsForWithdrawal(self.client_id))
        } else {
            let mut a = Account::default();
            a.client_id = self.client_id;
            a.available_amount = self.available_amount - amount;
            a.held_amount = self.held_amount;
            a.total_amount = a.available_amount + a.held_amount;
            Ok(a)
        }
    }

    fn dispute(
        &self,
        tx_id: TxId,
        history: &mut HashMap<TxId, Transaction>,
    ) -> core::result::Result<Self, AccountError> {
        if self.is_locked {
            return Err(AccountError::Frozen(self.client_id));
        }

        let t = history.get_mut(&tx_id);
        match t {
            Some(tx) => {
                tx.in_dispute = true;
                let mut a = Account::default();
                a.client_id = self.client_id;
                a.available_amount = self.available_amount - tx.amount;
                a.held_amount = self.held_amount + tx.amount;
                a.total_amount = a.available_amount + a.held_amount;
                Ok(a)
            }
            None => Err(AccountError::NoTxForDispute(tx_id)),
        }
    }

    fn resolve(
        &self,
        tx_id: TxId,
        history: &mut HashMap<TxId, Transaction>,
    ) -> core::result::Result<Self, AccountError> {
        if self.is_locked {
            return Err(AccountError::Frozen(self.client_id));
        }

        let t = history.get_mut(&tx_id);
        match t {
            Some(tx) => {
                if tx.in_dispute {
                    tx.in_dispute = false;
                    let mut a = Account::default();
                    a.client_id = self.client_id;
                    a.available_amount = self.available_amount + tx.amount;
                    a.held_amount = self.held_amount - tx.amount;
                    a.total_amount = a.available_amount + a.held_amount;
                    Ok(a)
                } else {
                    Err(AccountError::TxNotInDispute(tx_id))
                }
            }
            None => Err(AccountError::NoTxForDispute(tx_id)),
        }
    }

    fn chargeback(
        &self,
        tx_id: TxId,
        history: &mut HashMap<TxId, Transaction>,
    ) -> core::result::Result<Self, AccountError> {
        if self.is_locked {
            return Err(AccountError::Frozen(self.client_id));
        }
        let t = history.get_mut(&tx_id);
        match t {
            Some(tx) => {
                if tx.in_dispute {
                    tx.in_dispute = false;
                    let mut a = Account::default();
                    a.client_id = self.client_id;
                    a.available_amount = self.available_amount;
                    a.held_amount = self.held_amount - tx.amount;
                    a.total_amount = a.available_amount + a.held_amount;
                    a.is_locked = true;
                    Ok(a)
                } else {
                    Err(AccountError::TxNotInDispute(tx_id))
                }
            }
            None => Err(AccountError::NoTxForDispute(tx_id)),
        }
    }
}

#[derive(Debug)]
pub struct AccountProcess {
    pub client_id: ClientId,
    pub transactions: Sender<Option<Transaction>>,
}

/// Transaction processing functionality
pub struct TxProcessor {}

impl TxProcessor {
    ///
    ///
    /// 'mut rx'
    pub async fn process_transactions(
        mut tx_receiver: Receiver<Option<Transaction>>,
        buffer_size: usize,
    ) {
        let mut procs = HashMap::<ClientId, AccountProcess>::new();

        let mut tx_count = 0;

        while let Some(Some(t)) = tx_receiver.recv().await {
            tx_count += 1;
            trace!("processing tx {tx_count} {:?}", t);
            let link = procs.get_key_value(&t.client_id);
            match link {
                None => {
                    let (acc_tx_sender, acc_tx_receiver) =
                        tokio::sync::mpsc::channel::<Option<Transaction>>(buffer_size);
                    procs.insert(
                        t.client_id,
                        AccountProcess {
                            client_id: t.client_id,
                            transactions: acc_tx_sender.clone(),
                        },
                    );
                    tokio::spawn(async move {
                        TxProcessor::process_account_transactions(t.client_id, acc_tx_receiver)
                            .await;
                    });
                    let _ = acc_tx_sender.send(Some(t)).await;
                }
                Some((_k, proc)) => {
                    let _ = proc.transactions.send(Some(t)).await;
                }
            }
        }

        debug!("finished distributing transactions: shutting down account tasks");

        for p in procs.values() {
            let _ = p.transactions.send(Option::None).await;
            p.transactions.closed().await;
            trace!(
                "accountprocess {} tx is closed: {}",
                p.client_id,
                p.transactions.is_closed()
            );
        }

        debug!("all account processing tasks has been closed");
    }

    async fn process_account_transactions(id: ClientId, mut rx: Receiver<Option<Transaction>>) {
        let mut account = Account::default();
        account.client_id = id;

        debug!("created account {:?}", &account);

        //local history of transactions made to this account
        let mut transactions = HashMap::<u32, Transaction>::new();

        while let Some(Some(t)) = rx.recv().await {
            trace!("account {} processing {:?}", account.client_id, t);
            let r = account.process_transaction(&t, &mut transactions);
            match r {
                Ok(a) => account = a,
                Err(e) => {
                    info!("{:?}", e);
                }
            }
            if t.tx_type == TxType::Deposit || t.tx_type == TxType::Withdrawal {
                transactions.insert(t.tx_id, t);
            }

            trace!("account state: {:?}", &account);
        }

        debug!("{:?}", account);

        println!(
            "{0},{1:.4},{2:.4},{3:.4},{4}",
            account.client_id,
            account.available_amount,
            account.held_amount,
            account.total_amount,
            account.is_locked
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::tx::{Account, Transaction};

    /// tests for default settings
    #[test]
    fn account_default() {
        let a = Account::default();
        assert_eq!(a.client_id, 0);
        assert_eq!(a.available_amount, 0.0);
        assert_eq!(a.held_amount, 0.0);
        assert_eq!(a.total_amount, 0.0);
        assert_eq!(a.is_locked, false);
    }

    #[test]
    fn account_deposit() {
        let mut a = Account {
            client_id: 1,
            total_amount: 0.0,
            held_amount: 0.0,
            available_amount: 0.0,
            is_locked: false,
        };
        let a1 = a.deposit(5.0).unwrap();
        a = Account {
            client_id: 1,
            total_amount: 5.0,
            held_amount: 0.0,
            available_amount: 5.0,
            is_locked: false,
        };

        assert_eq!(a, a1);
    }

    #[test]
    fn account_withdrawal() {
        let mut a = Account {
            client_id: 1,
            total_amount: 15.0,
            held_amount: 5.0,
            available_amount: 10.0,
            is_locked: false,
        };
        let a1 = a.withdrawal(5.0).unwrap();
        a = Account {
            client_id: 1,
            total_amount: 10.0,
            held_amount: 5.0,
            available_amount: 5.0,
            is_locked: false,
        };

        assert_eq!(a, a1);
    }

    #[test]
    fn account_dispute() {
        let mut a = Account {
            client_id: 1,
            available_amount: 10.0,
            held_amount: 5.0,
            total_amount: 15.0,
            is_locked: false,
        };
        let mut history = HashMap::<u32, Transaction>::new();
        history.insert(
            1,
            Transaction {
                tx_type: crate::tx::TxType::Deposit,
                client_id: 1,
                tx_id: 1,
                amount: 10.0,
                in_dispute: false,
            },
        );
        let a1 = a.dispute(1, &mut history).unwrap();
        a = Account {
            client_id: 1,
            available_amount: 0.0,
            held_amount: 15.0,
            total_amount: 15.0,
            is_locked: false,
        };

        assert_eq!(a, a1);
    }

    #[test]
    fn account_resolve() {
        let mut a = Account {
            client_id: 1,
            available_amount: 0.0,
            held_amount: 15.0,
            total_amount: 15.0,
            is_locked: false,
        };
        let mut history = HashMap::<u32, Transaction>::new();
        history.insert(
            1,
            Transaction {
                tx_type: crate::tx::TxType::Deposit,
                client_id: 1,
                tx_id: 1,
                amount: 10.0,
                in_dispute: true,
            },
        );
        let a1 = a.resolve(1, &mut history).unwrap();
        a = Account {
            client_id: 1,
            available_amount: 10.0,
            held_amount: 5.0,
            total_amount: 15.0,
            is_locked: false,
        };

        assert_eq!(a, a1);
    }

    #[test]
    fn account_chargeback() {
        let mut a = Account {
            client_id: 1,
            available_amount: 10.0,
            held_amount: 15.0,
            total_amount: 25.0,
            is_locked: false,
        };
        let mut history = HashMap::<u32, Transaction>::new();
        history.insert(
            1,
            Transaction {
                tx_type: crate::tx::TxType::Deposit,
                client_id: 1,
                tx_id: 1,
                amount: 10.0,
                in_dispute: true,
            },
        );
        let a1 = a.chargeback(1, &mut history).unwrap();
        a = Account {
            client_id: 1,
            available_amount: 10.0,
            held_amount: 5.0,
            total_amount: 15.0,
            is_locked: true,
        };

        assert_eq!(a, a1);
    }
}
