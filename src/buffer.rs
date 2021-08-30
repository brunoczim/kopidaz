use crate::{encode_into, error::Error};
use std::cell::Cell;

#[derive(Debug, Default)]
pub struct Buffer {
    bytes: Vec<u8>,
}

impl Buffer {
    pub fn encode<T>(&mut self, data: T) -> Result<&[u8], Error>
    where
        T: serde::Serialize,
    {
        self.bytes.clear();
        encode_into(data, &mut self.bytes)?;
        Ok(&self.bytes)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn free(&mut self) {
        *self = Self::default();
    }
}

impl Clone for Buffer {
    fn clone(&self) -> Self {
        Self::default()
    }
}

pub trait Allocation {
    fn get(&mut self) -> Buffer;
    fn save(&mut self, buffer: Buffer);
    fn free(&mut self);
}

impl<'this, A> Allocation for &'this mut A
where
    A: Allocation,
{
    fn get(&mut self) -> Buffer {
        (**self).get()
    }

    fn save(&mut self, buffer: Buffer) {
        (**self).save(buffer)
    }

    fn free(&mut self) {
        (**self).free()
    }
}

#[derive(Debug, Clone, Default)]
pub struct OneTime;

impl Allocation for OneTime {
    fn get(&mut self) -> Buffer {
        Buffer::default()
    }

    fn save(&mut self, _buffer: Buffer) {}

    fn free(&mut self) {}
}

#[derive(Debug, Default)]
pub struct Pool {
    buffers: Vec<Buffer>,
}

impl Allocation for Pool {
    fn get(&mut self) -> Buffer {
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

#[derive(Debug, Clone, Default)]
pub struct DefaultPool;

impl Allocation for DefaultPool {
    fn get(&mut self) -> Buffer {
        with_default_pool(Pool::get)
    }

    fn save(&mut self, buffer: Buffer) {
        with_default_pool(|pool| pool.save(buffer))
    }

    fn free(&mut self) {
        with_default_pool(Pool::free)
    }
}
