//! This crate provides a wrapper over sled for typed trees. WIP.

pub mod error;
pub mod buffer;
pub mod tree;

use crate::error::Error;
use bincode::Options;
use std::path::Path;
use tokio::task;

/// Opens a database in the given path.
pub async fn open<P>(path: P) -> Result<sled::Db, Error>
where
    P: AsRef<Path>,
{
    task::block_in_place(|| Ok(sled::open(path)?))
}

/// Default configs for bincode.
fn config() -> impl Options {
    bincode::DefaultOptions::new().with_no_limit().with_big_endian()
}

/// Encodes a value into binary, using the given buffer.
pub fn encode_into<T>(data: T, buffer: &mut Vec<u8>) -> Result<(), Error>
where
    T: serde::Serialize,
{
    config().serialize_into(buffer, &data)?;
    Ok(())
}

/// Encodes a value into binary, allocating a new buffer.
pub fn encode<T>(data: T) -> Result<Vec<u8>, Error>
where
    T: serde::Serialize,
{
    let mut buffer = Vec::new();
    encode_into(data, &mut buffer)?;
    Ok(buffer)
}

/// Decodes a value from binary.
pub fn decode<'de, T>(bytes: &'de [u8]) -> Result<T, Error>
where
    T: serde::Deserialize<'de>,
{
    let data = config().deserialize(bytes)?;
    Ok(data)
}
