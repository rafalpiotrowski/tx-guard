#![deny(warnings)]

/// Error returned by most functions.
///
/// todo: we might want to use specialized error handling crate or defining an error type as an `enum` of causes.
/// However, for our example, using a boxed `std::error::Error` is sufficient.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// A specialized `Result` type for transaction processing operations.
///
/// This is defined as a convenience.
pub type Result<T> = std::result::Result<T, Error>;

/// Client's ID type alias
pub type ClientId = u16;

/// Transaction ID type alias
pub type TxId = u32;

/// alias for money type
pub type Money = f32;

/// Transaction types
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TxType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

/// Transaction data
#[derive(Debug, Clone)]
pub struct Transaction {
    pub tx_type: TxType,
    pub client_id: ClientId,
    pub tx_id: TxId,
    pub amount: Money,
    pub in_dispute: bool,
}

// exposing tx module to be used by clients
pub mod tx;

#[macro_use]
extern crate serde;
// expose this module for clients
pub mod csv;

// we do not need to expose this module for external use
mod account;