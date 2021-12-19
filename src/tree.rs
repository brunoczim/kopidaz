//! Exports a persistent, serializing/deserializing ordered tree.

use crate::{
    buffer::{self, Buffer},
    decode,
    error::Error,
};
use futures::future::{FutureExt, Map};
use std::{fmt, future, future::Future, marker::PhantomData};
use tokio::task;

/// An ID generated by the tree.
pub type Id = u64;

/// A persistent key-value structure.
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

    async fn get_raw(
        &self,
        key: &K,
        key_buf: &mut Buffer,
    ) -> Result<Option<V>, Error> {
        let encoded_key = key_buf.encode(key)?;
        let maybe = task::block_in_place(|| self.storage.get(&encoded_key))?;
        match maybe {
            Some(encoded_value) => {
                let val = decode(&encoded_value)?;
                Ok(Some(val))
            },
            None => Ok(None),
        }
    }

    /// Gets the value associated with the given `key`, returning `None` if key
    /// is not found. Serializes key using a buffer from a thread-local buffer
    /// pool.
    pub async fn get(&self, key: &K) -> Result<Option<V>, Error> {
        self.get_with(key, buffer::DefaultPool).await
    }

    /// Gets the value associated with the given `key`, returning `None` if key
    /// is not found. Uses the given allocation strategy for making buffers.
    pub async fn get_with<A>(
        &self,
        key: &K,
        mut allocation: A,
    ) -> Result<Option<V>, Error>
    where
        A: buffer::Allocation,
    {
        let mut key_buf = allocation.make();
        let result = self.get_raw(key, &mut key_buf).await;
        allocation.save(key_buf);
        result
    }

    async fn insert_raw(
        &self,
        key: &K,
        val: &V,
        key_buf: &mut Buffer,
        val_buf: &mut Buffer,
    ) -> Result<Option<V>, Error> {
        let encoded_key = key_buf.encode(key)?;
        let encoded_value = val_buf.encode(val)?;
        let encoded = task::block_in_place(|| {
            self.storage.insert(&encoded_key, encoded_value)
        })?;
        match encoded {
            Some(encoded_val) => Ok(Some(decode(&encoded_val)?)),
            None => Ok(None),
        }
    }

    /// Inserts key and value returning `None` if key is new, `Some(old_value)`
    /// if the key already exists (and replacing its data). Serializes key and
    /// value using a buffer from a thread-local buffer pool.
    pub async fn insert(&self, key: &K, val: &V) -> Result<Option<V>, Error> {
        self.insert_with(key, val, buffer::DefaultPool).await
    }

    /// Inserts key and value returning `None` if key is new, `Some(old_value)`
    /// if the key already exists (and replacing its data). Uses the given
    /// allocation strategy for making buffers.
    pub async fn insert_with<A>(
        &self,
        key: &K,
        val: &V,
        mut allocation: A,
    ) -> Result<Option<V>, Error>
    where
        A: buffer::Allocation,
    {
        let mut key_buf = allocation.make();
        let mut val_buf = allocation.make();
        let result =
            self.insert_raw(key, val, &mut key_buf, &mut val_buf).await;
        allocation.save(key_buf);
        allocation.save(val_buf);
        result
    }

    async fn contains_key_raw(
        &self,
        key: &K,
        key_buf: &mut Buffer,
    ) -> Result<bool, Error> {
        let encoded_key = key_buf.encode(key)?;
        let result =
            task::block_in_place(|| self.storage.contains_key(&encoded_key))?;
        Ok(result)
    }

    /// Tests if the given key exist. Serializes key using a buffer from a
    /// thread-local buffer pool.
    pub async fn contains_key(&self, key: &K) -> Result<bool, Error> {
        self.contains_key_with(key, buffer::DefaultPool).await
    }

    /// Tests if the given key exist. Uses the given allocation strategy for
    /// making buffers.
    pub async fn contains_key_with<A>(
        &self,
        key: &K,
        mut allocation: A,
    ) -> Result<bool, Error>
    where
        A: buffer::Allocation,
    {
        let mut key_buf = allocation.make();
        let result = self.contains_key_raw(key, &mut key_buf).await;
        allocation.save(key_buf);
        result
    }

    async fn remove_raw(
        &self,
        key: &K,
        key_buf: &mut Buffer,
    ) -> Result<Option<V>, Error> {
        let encoded_key = key_buf.encode(key)?;
        match task::block_in_place(|| self.storage.remove(&encoded_key))? {
            Some(encoded_val) => Ok(Some(decode(&encoded_val)?)),
            None => Ok(None),
        }
    }

    /// Removes the value associated with the given `key`, returning `None` if
    /// key is not found. Serializes key using a buffer from a thread-local
    /// buffer pool.
    pub async fn remove(&self, key: &K) -> Result<Option<V>, Error> {
        self.remove_with(key, buffer::DefaultPool).await
    }

    /// Removes the value associated with the given `key`, returning `None` if
    /// key is not found. Uses the given allocation strategy for making buffers.
    pub async fn remove_with<A>(
        &self,
        key: &K,
        mut allocation: A,
    ) -> Result<Option<V>, Error>
    where
        A: buffer::Allocation,
    {
        let mut key_buf = allocation.make();
        let result = self.remove_raw(key, &mut key_buf).await;
        allocation.save(key_buf);
        result
    }

    /// Creates a builder for an ID generator.
    ///
    /// An ID generator tries to generate a new ID as a key of an entry, and
    /// when successful, inserts the key with a value. First of all, it
    /// generates an integer, and then it uses the given function `make_id` to
    /// produce a key. When an actual such key is indeed new, the method uses
    /// another function, `make_data`, to produce a value associated with the
    /// key. With a key-value pair, it inserts them in the tree.
    ///
    /// Serializes key and value using thread-local buffer by default, but
    /// allows passing a custom allocation. Also by default, all errors could
    /// only be [`Error`], but that behaviour is configurable via
    /// [`IdBuilder::error_conversor`];
    pub fn id_builder(
        &self,
    ) -> IdBuilder<K, V, buffer::DefaultPool, fn(Error) -> Error, (), ()> {
        IdBuilder::new(self)
    }
}

impl<K, V> Clone for Tree<K, V>
where
    for<'de> K: serde::Serialize + serde::Deserialize<'de>,
    for<'de> V: serde::Serialize + serde::Deserialize<'de>,
{
    fn clone(&self) -> Self {
        Self { _marker: self._marker, storage: self.storage.clone() }
    }
}

impl<K, V> fmt::Debug for Tree<K, V>
where
    for<'de> K: serde::Serialize + serde::Deserialize<'de>,
    for<'de> V: serde::Serialize + serde::Deserialize<'de>,
{
    fn fmt(&self, fmtr: &mut fmt::Formatter) -> fmt::Result {
        fmtr.debug_struct("Tree").field("storage", &self.storage).finish()
    }
}

/// An ID generator builder. See [`Tree::id_builder`] for more details.
#[derive(Debug, Clone)]
pub struct IdBuilder<'tree, K, V, A, FE, FK, FV>
where
    for<'de> K: serde::Serialize + serde::Deserialize<'de>,
    for<'de> V: serde::Serialize + serde::Deserialize<'de>,
{
    tree: &'tree Tree<K, V>,
    allocation: A,
    make_error: FE,
    make_id: FK,
    make_data: FV,
}

impl<'tree, K, V>
    IdBuilder<'tree, K, V, buffer::DefaultPool, fn(Error) -> Error, (), ()>
where
    for<'de> K: serde::Serialize + serde::Deserialize<'de>,
    for<'de> V: serde::Serialize + serde::Deserialize<'de>,
{
    fn new(tree: &'tree Tree<K, V>) -> Self {
        Self {
            tree,
            allocation: buffer::DefaultPool,
            make_error: |error| error,
            make_id: (),
            make_data: (),
        }
    }
}

impl<'tree, K, V, A, FE, FK, FV> IdBuilder<'tree, K, V, A, FE, FK, FV>
where
    for<'de> K: serde::Serialize + serde::Deserialize<'de>,
    for<'de> V: serde::Serialize + serde::Deserialize<'de>,
{
    /// Changes the serialization buffer allocation. By default, the builder
    /// would use a thread-local pool.
    pub fn allocation<A0>(
        self,
        allocation: A0,
    ) -> IdBuilder<'tree, K, V, A0, FE, FK, FV>
    where
        A0: buffer::Allocation,
    {
        IdBuilder {
            tree: self.tree,
            allocation,
            make_error: self.make_error,
            make_id: self.make_id,
            make_data: self.make_data,
        }
    }

    /// Sets the error conversor (a function).
    pub fn error_conversor<FE0, E>(
        self,
        make_error: FE0,
    ) -> IdBuilder<'tree, K, V, A, FE0, FK, FV>
    where
        FE0: FnOnce(Error) -> E,
    {
        IdBuilder {
            tree: self.tree,
            allocation: self.allocation,
            make_error,
            make_id: self.make_id,
            make_data: self.make_data,
        }
    }

    /// Sets the error conversor simply as the implementation of the `From`
    /// trait.
    pub fn error_from<E>(
        self,
    ) -> IdBuilder<'tree, K, V, A, impl FnOnce(Error) -> E, FK, FV>
    where
        E: From<Error>,
    {
        self.error_conversor(E::from)
    }

    /// Sets the given function as the "id maker", a function that CANNOT fail
    /// and is SYNChronous.
    pub fn id_maker<FK0, E>(
        self,
        mut make_id: FK0,
    ) -> IdBuilder<
        'tree,
        K,
        V,
        A,
        FE,
        impl FnMut(Id) -> future::Ready<Result<K, E>>,
        FV,
    >
    where
        FK0: FnMut(Id) -> K,
    {
        self.fallible_async_id_maker(move |bits| {
            future::ready(Ok(make_id(bits)))
        })
    }

    /// Sets the given function as the "id maker", a function that CAN fail
    /// and is SYNChronous.
    pub fn fallible_id_maker<FK0, E>(
        self,
        mut make_id: FK0,
    ) -> IdBuilder<
        'tree,
        K,
        V,
        A,
        FE,
        impl FnMut(Id) -> future::Ready<Result<K, E>>,
        FV,
    >
    where
        FK0: FnMut(Id) -> Result<K, E>,
    {
        self.fallible_async_id_maker(move |bits| future::ready(make_id(bits)))
    }

    /// Sets the given function as the "id maker", a function that CANNOT fail
    /// and is ASYNChronous.
    pub fn async_id_maker<FK0, AK, E>(
        self,
        mut make_id: FK0,
    ) -> IdBuilder<
        'tree,
        K,
        V,
        A,
        FE,
        impl FnMut(Id) -> Map<AK, fn(K) -> Result<K, E>>,
        FV,
    >
    where
        FK0: FnMut(Id) -> AK,
        AK: Future<Output = K>,
    {
        self.fallible_async_id_maker(move |bits| {
            make_id(bits).map(Ok as fn(K) -> Result<K, E>)
        })
    }

    /// Sets the given function as the "id maker", a function that CAN fail
    /// and is ASYNChronous.
    pub fn fallible_async_id_maker<FK0, AK, E>(
        self,
        make_id: FK0,
    ) -> IdBuilder<'tree, K, V, A, FE, FK0, FV>
    where
        FK0: FnMut(Id) -> AK,
        AK: Future<Output = Result<K, E>>,
    {
        IdBuilder {
            tree: self.tree,
            allocation: self.allocation,
            make_error: self.make_error,
            make_id,
            make_data: self.make_data,
        }
    }

    /// Sets the given function as the "data maker", a function that CANNOT fail
    /// and is SYNChronous.
    pub fn data_maker<FV0, E>(
        self,
        make_data: FV0,
    ) -> IdBuilder<
        'tree,
        K,
        V,
        A,
        FE,
        FK,
        impl FnOnce(&Id) -> future::Ready<Result<V, E>>,
    >
    where
        FV0: FnOnce(&Id) -> V,
    {
        self.fallible_async_data_maker(move |id| {
            future::ready(Ok(make_data(id)))
        })
    }

    /// Sets the given function as the "data maker", a function that CAN fail
    /// and is SYNChronous.
    pub fn fallible_data_maker<FV0, E>(
        self,
        make_data: FV0,
    ) -> IdBuilder<
        'tree,
        K,
        V,
        A,
        FE,
        FK,
        impl FnOnce(&Id) -> future::Ready<Result<V, E>>,
    >
    where
        FV0: FnOnce(&Id) -> Result<V, E>,
    {
        self.fallible_async_data_maker(move |id| future::ready(make_data(id)))
    }

    /// Sets the given function as the "data maker", a function that CANNOT fail
    /// and is ASYNChronous.
    pub fn async_data_maker<FV0, AV, E>(
        self,
        make_data: FV0,
    ) -> IdBuilder<
        'tree,
        K,
        V,
        A,
        FE,
        FK,
        impl FnOnce(&Id) -> Map<AV, fn(V) -> Result<V, E>>,
    >
    where
        FV0: FnOnce(&Id) -> AV,
        AV: Future<Output = V>,
    {
        self.fallible_async_data_maker(move |bits| {
            make_data(bits).map(Ok as fn(V) -> Result<V, E>)
        })
    }

    /// Sets the given function as the "data maker", a function that CAN fail
    /// and is ASYNChronous.
    pub fn fallible_async_data_maker<FV0, AV, E>(
        self,
        make_data: FV0,
    ) -> IdBuilder<'tree, K, V, A, FE, FK, FV0>
    where
        FV0: FnOnce(&Id) -> AV,
        AV: Future<Output = Result<V, E>>,
    {
        IdBuilder {
            tree: self.tree,
            allocation: self.allocation,
            make_error: self.make_error,
            make_id: self.make_id,
            make_data,
        }
    }

    /// Generates the ID whenever the builder is ready. The builder is ready if
    /// all of "error conversor", "id maker" and "data maker". Meanwhile,
    /// "allocator" has a default value.
    pub async fn generate<E, AK, AV>(
        mut self,
        db: &sled::Db,
    ) -> Result<(K, V), E>
    where
        A: buffer::Allocation,
        FE: FnOnce(Error) -> E,
        FK: FnMut(Id) -> AK,
        AK: Future<Output = Result<K, E>>,
        FV: FnOnce(&K) -> AV,
        AV: Future<Output = Result<V, E>>,
    {
        let mut key_buf = self.allocation.make();
        let mut val_buf = self.allocation.make();

        let output = loop {
            let generated = match task::block_in_place(|| db.generate_id()) {
                Ok(id) => id,
                Err(error) => break Err((self.make_error)(error.into())),
            };
            let id = match (self.make_id)(generated).await {
                Ok(id) => id,
                Err(error) => break Err(error),
            };

            let contains =
                match self.tree.contains_key_raw(&id, &mut key_buf).await {
                    Ok(contains) => contains,
                    Err(error) => break Err((self.make_error)(error.into())),
                };

            if !contains {
                let data = match (self.make_data)(&id).await {
                    Ok(data) => data,
                    Err(error) => break Err(error),
                };
                if let Err(error) = self
                    .tree
                    .insert_raw(&id, &data, &mut key_buf, &mut val_buf)
                    .await
                {
                    break Err((self.make_error)(error.into()));
                }

                break Ok((id, data));
            }

            task::yield_now().await;
        };

        self.allocation.save(key_buf);
        self.allocation.save(val_buf);

        output
    }
}
