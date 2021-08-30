//! This module defines utilites for encoding buffers, which target better
//! performances by not discarding allocations.

use crate::{encode_into, error::Error};
use std::cell::Cell;

/// An encode buffer. Useful for not throwing away allocations.
#[derive(Debug, Default)]
pub struct Buffer {
    bytes: Vec<u8>,
}

impl Buffer {
    /// Encodes the given input data.
    pub fn encode<T>(&mut self, data: T) -> Result<&[u8], Error>
    where
        T: serde::Serialize,
    {
        self.bytes.clear();
        encode_into(data, &mut self.bytes)?;
        Ok(&self.bytes)
    }

    /// Returns the last encoded bytes. Initially, this is just an empty slice.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Frees all the memory of this buffer.
    pub fn free(&mut self) {
        *self = Self::default();
    }
}

impl Clone for Buffer {
    fn clone(&self) -> Self {
        Self::default()
    }
}

/// Allocation strategy for encode buffers.
pub trait Allocation {
    /// Allocates a buffer.
    fn make(&mut self) -> Buffer;

    /// Saves an allocated buffer.
    fn save(&mut self, buffer: Buffer);

    /// Frees all allocated buffers.
    fn free(&mut self);
}

impl<'this, A> Allocation for &'this mut A
where
    A: Allocation,
{
    fn make(&mut self) -> Buffer {
        (**self).make()
    }

    fn save(&mut self, buffer: Buffer) {
        (**self).save(buffer)
    }

    fn free(&mut self) {
        (**self).free()
    }
}

/// Allocator for one time use buffers. Saving buffers through this does
/// nothing.
#[derive(Debug, Clone, Default)]
pub struct OneTime;

impl Allocation for OneTime {
    fn make(&mut self) -> Buffer {
        Buffer::default()
    }

    fn save(&mut self, _buffer: Buffer) {}

    fn free(&mut self) {}
}

/// A pool for buffer allocations. Saves all allocated buffers.
#[derive(Debug, Default)]
pub struct Pool {
    buffers: Vec<Buffer>,
}

impl Allocation for Pool {
    fn make(&mut self) -> Buffer {
        self.buffers.pop().unwrap_or_else(Buffer::default)
    }

    fn save(&mut self, buffer: Buffer) {
        self.buffers.push(buffer);
    }

    fn free(&mut self) {
        *self = Self::default();
    }
}

impl Clone for Pool {
    fn clone(&self) -> Self {
        Self::default()
    }
}

thread_local! {
    static DEFAULT_POOL: Cell<Pool> = Cell::new(Pool::default());
}

fn with_default_pool<F, T>(visitor: F) -> T
where
    F: FnOnce(&mut Pool) -> T,
{
    DEFAULT_POOL.with(|cell| {
        let mut pool = cell.take();
        let ret = visitor(&mut pool);
        cell.set(pool);
        ret
    })
}

/// Default pool for buffer allocations implemented as a thread-local pool of
/// buffers.
#[derive(Debug, Clone, Default)]
pub struct DefaultPool;

impl Allocation for DefaultPool {
    fn make(&mut self) -> Buffer {
        with_default_pool(Pool::make)
    }

    fn save(&mut self, buffer: Buffer) {
        with_default_pool(|pool| pool.save(buffer))
    }

    fn free(&mut self) {
        with_default_pool(Pool::free)
    }
}
