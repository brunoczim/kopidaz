use crate::{decode, error::Error, EncodeBuffer};
use std::{future::Future, marker::PhantomData};
use tokio::task;

pub type Id = u64;

/// A persistent key-value structure.
#[derive(Debug)]
pub struct Tree<K, V>
where
    for<'de> K: serde::Serialize + serde::Deserialize<'de>,
    for<'de> V: serde::Serialize + serde::Deserialize<'de>,
{
    storage: sled::Tree,
    _marker: PhantomData<(K, V)>,
}

impl<K, V> Tree<K, V>
where
    for<'de> K: serde::Serialize + serde::Deserialize<'de>,
    for<'de> V: serde::Serialize + serde::Deserialize<'de>,
{
    /// Opens this tree from a database.
    pub async fn open<T>(db: &sled::Db, name: T) -> Result<Self, Error>
    where
        T: AsRef<[u8]>,
    {
        let storage = task::block_in_place(|| db.open_tree(name))?;
        Ok(Self { storage, _marker: PhantomData })
    }

    /// Gets a value associated with a given key using the given allocated
    /// buffer to serialize and dserialize key and value.
    pub async fn get_with_buf(
        &self,
        key: &K,
        buffer: &mut EncodeBuffer,
    ) -> Result<Option<V>, Error> {
        let encoded_key = buffer.encode_key(key)?;
        let maybe = task::block_in_place(|| self.storage.get(&encoded_key))?;
        match maybe {
            Some(encoded_value) => {
                let val = decode(&encoded_value)?;
                Ok(Some(val))
            },
            None => Ok(None),
        }
    }

    /// Gets a value associated with a given key creating a one-time use buffer.
    pub async fn get(&self, key: &K) -> Result<Option<V>, Error> {
        self.get_with_buf(key, &mut EncodeBuffer::default()).await
    }

    /// Inserts a value associated with a given key using the given allocated
    /// buffer to serialize and dserialize key and value.
    pub async fn insert_with_buf(
        &self,
        key: &K,
        val: &V,
        buffer: &mut EncodeBuffer,
    ) -> Result<(), Error> {
        let (encoded_key, encoded_value) = buffer.encode(key, val)?;
        task::block_in_place(|| {
            self.storage.insert(&encoded_key, encoded_value)
        })?;
        Ok(())
    }

    /// Inserts a value associated with a given key creating a one-time use
    /// buffer.
    pub async fn insert(&self, key: &K, val: &V) -> Result<(), Error> {
        self.insert_with_buf(key, val, &mut EncodeBuffer::default()).await
    }

    /// Returns whether the given key is present in this tree using the given
    /// allocated buffer to serialize and dserialize key and value.
    pub async fn contains_key_with_buf(
        &self,
        key: &K,
        buffer: &mut EncodeBuffer,
    ) -> Result<bool, Error> {
        let encoded_key = buffer.encode_key(key)?;
        let result =
            task::block_in_place(|| self.storage.contains_key(&encoded_key))?;
        Ok(result)
    }

    /// Returns whether the given key is present in this tree creating a
    /// one-time use buffer.
    pub async fn contains_key(&self, key: &K) -> Result<bool, Error> {
        self.contains_key_with_buf(key, &mut EncodeBuffer::default()).await
    }

    /// Tries to generate an ID until it is successful. The ID is stored
    /// alongside with a value in a given tree using the given allocated buffer
    /// to serialize and dserialize key and value.
    pub async fn generate_id_with_buf<FK, FV, AK, AV, E>(
        &self,
        db: &sled::Db,
        buffer: &mut EncodeBuffer,
        mut make_id: FK,
        make_data: FV,
    ) -> Result<K, E>
    where
        FK: FnMut(Id) -> AK,
        FV: FnOnce(&K) -> AV,
        AK: Future<Output = Result<K, E>>,
        AV: Future<Output = Result<V, E>>,
        E: From<Error>,
    {
        loop {
            let gen_result = task::block_in_place(|| db.generate_id());
            let generated = gen_result.map_err(Error::from)?;
            let id = make_id(generated).await?;

            let contains = self.contains_key_with_buf(&id, buffer).await?;

            if !contains {
                let data = make_data(&id).await?;
                self.insert_with_buf(&id, &data, buffer).await?;
                break Ok(id);
            }

            task::yield_now().await;
        }
    }

    /// Tries to generate an ID until it is successful. The ID is stored
    /// alongside with a value in a given tree creating a one-time use buffer.
    pub async fn generate_id<FK, FV, AK, AV, E>(
        &self,
        db: &sled::Db,
        make_id: FK,
        make_data: FV,
    ) -> Result<K, E>
    where
        FK: FnMut(Id) -> AK,
        FV: FnOnce(&K) -> AV,
        AK: Future<Output = Result<K, E>>,
        AV: Future<Output = Result<V, E>>,
        E: From<Error>,
    {
        let mut buffer = &mut EncodeBuffer::default();
        self.generate_id_with_buf(db, &mut buffer, make_id, make_data).await
    }
}
