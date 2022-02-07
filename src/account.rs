use std::collections::HashMap;

use crate::{ClientId, TxId, Money, TxType, Transaction};
use crate::csv::RawAccount;

/// Error types return when processing account's transaction
#[derive(Debug)]
pub enum AccountError {
    // Account is frozen, cannot perform any other operation on it
    Frozen(ClientId),
    InssuficientFundsForWithdrawal(ClientId),
    NoTxForDispute(TxId),
    TxNotInDispute(TxId),
}

/// data structure representing account state
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

/// converstion from RawAccount to Account
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
    /// call by the account transaction processing task to handle supplied transaction
    /// the only side effect can be on a transaction in the history, when we need to change the state of in_dispute
    /// due to dispute/resolve/chargeback events
    /// 
    /// `t` reference to transaction that is currently processed 
    /// `history` mutable reference to the history of all transaction for given account
    /// 
    /// return new Account instrance
    /// 
    /// todo: improvement could be done in order to make this pure function. 
    /// One ide is to return info that another transaction should be changed
    pub(crate) fn process_transaction(
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

    /// A deposit is a credit to the client's asset account, meaning it should increase the available and
    /// total funds of the client account
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

    /// A withdraw is a debit to the client's asset account, meaning it should decrease the available and
    /// total funds of the client account
    /// If a client does not have sufficient available funds the withdrawal should fail and the total amount
    /// of funds should not change
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

    /// A dispute represents a client's claim that a transaction was erroneous and should be reversed.
    /// The transaction shouldn't be reversed yet but the associated funds should be held. This means
    /// that the clients available funds should decrease by the amount disputed, their held funds should
    /// increase by the amount disputed, while their total funds should remain the same.
    /// Notice that a dispute does not state the amount disputed. Instead a dispute references the
    /// transaction that is disputed by ID. If the tx specified by the dispute doesn't exist you can ignore it
    /// and assume this is an error on our partners side.
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

    /// A resolve represents a resolution to a dispute, releasing the associated held funds. Funds that
    /// were previously disputed are no longer disputed. This means that the clients held funds should
    /// decrease by the amount no longer disputed, their available funds should increase by the
    /// amount no longer disputed, and their total funds should remain the same.
    /// Like disputes, resolves do not specify an amount. Instead they refer to a transaction that was
    /// under dispute by ID. If the tx specified doesn't exist, or the tx isn't under dispute, you can ignore
    /// the resolve and assume this is an error on our partner's side.
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

    /// A chargeback is the final state of a dispute and represents the client reversing a transaction.
    /// Funds that were held have now been withdrawn. This means that the clients held funds and
    /// total funds should decrease by the amount previously disputed. If a chargeback occurs the
    /// client's account should be immediately frozen.
    /// Like a dispute and a resolve a chargeback refers to the transaction by ID (tx) and does not
    /// specify an amount. Like a resolve, if the tx specified doesn't exist, or the tx isn't under dispute,
    /// you can ignore chargeback and assume this is an error on our partner's side.
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{account::Account, TxType, Transaction};

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
                tx_type: TxType::Deposit,
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
                tx_type: TxType::Deposit,
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
                tx_type: TxType::Deposit,
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
