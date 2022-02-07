#![deny(warnings)]

/// Default size of the channel buffer
///
pub const DEFAULT_CHANNEL_SIZE: u8 = 32;

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

/// Client's ID type alias
pub type ClientId = u16;

/// Transaction ID type alias
pub type TxId = u32;

/// alias for money type
pub type Money = f32;

pub mod tx;

#[macro_use]
extern crate serde;

pub mod csv;