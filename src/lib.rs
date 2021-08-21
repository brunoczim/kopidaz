pub mod error;
pub mod tree;

use crate::error::Error;
use bincode::Options;

/// Default configs for bincode.
fn config() -> impl Options {
    bincode::DefaultOptions::new().with_no_limit().with_big_endian()
}

/// Encodes a value into binary.
pub fn encode<T>(data: T, buffer: &mut Vec<u8>) -> Result<(), Error>
where
    T: serde::Serialize,
{
    config().serialize_into(buffer, &data)?;
    Ok(())
}

/// Decodes a value from binary.
pub fn decode<'de, T>(bytes: &'de [u8]) -> Result<T, Error>
where
    T: serde::Deserialize<'de>,
{
    let data = config().deserialize(bytes)?;
    Ok(data)
}
