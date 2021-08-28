pub mod error;
pub mod tree;

use crate::error::Error;
use bincode::Options;

#[derive(Debug, Default)]
pub struct EncodeBuffer {
    key: Vec<u8>,
    value: Vec<u8>,
}

impl EncodeBuffer {
    pub fn free_key(&mut self) {
        self.key = Vec::new();
    }

    pub fn free_value(&mut self) {
        self.value = Vec::new();
    }

    pub fn encode_key<K>(&mut self, key: K) -> Result<&[u8], Error>
    where
        K: serde::Serialize,
    {
        self.key.clear();
        encode(key, &mut self.key)?;
        Ok(&self.key)
    }

    pub fn encode_value<V>(&mut self, value: V) -> Result<&[u8], Error>
    where
        V: serde::Serialize,
    {
        self.value.clear();
        encode(value, &mut self.value)?;
        Ok(&self.value)
    }

    pub fn encode<K, V>(
        &mut self,
        key: K,
        value: V,
    ) -> Result<(&[u8], &[u8]), Error>
    where
        K: serde::Serialize,
        V: serde::Serialize,
    {
        self.encode_key(key)?;
        self.encode_value(value)?;
        Ok((&self.key, &self.value))
    }

    pub fn last_key(&self) -> &[u8] {
        &self.key
    }

    pub fn last_value(&self) -> &[u8] {
        &self.value
    }
}

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
