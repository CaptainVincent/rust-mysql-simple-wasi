// Copyright (c) 2021 Anatoly Ikorsky
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use crossbeam::queue::ArrayQueue;

use std::{mem::replace, ops::Deref, sync::Arc};

const DEFAULT_MYSQL_BUFFER_POOL_CAP: usize = 128;
const DEFAULT_MYSQL_BUFFER_SIZE_CAP: usize = 4 * 1024 * 1024;

#[derive(Debug)]
struct Inner {
    buffer_cap: usize,
    pool: ArrayQueue<Vec<u8>>,
}

impl Inner {
    fn get(self: &Arc<Self>) -> PooledBuf {
        let mut buf = self.pool.pop().unwrap_or_default();

        // SAFETY:
        // 1. OK – 0 is always within capacity
        // 2. OK - nothing to initialize
        unsafe { buf.set_len(0) }

        PooledBuf(buf, Some(self.clone()))
    }

    fn put(&self, mut buf: Vec<u8>) {
        buf.shrink_to(self.buffer_cap);
        let _ = self.pool.push(buf);
    }
}

/// Smart pointer to a buffer pool.
#[derive(Debug, Clone)]
pub struct BufferPool(Option<Arc<Inner>>);

impl BufferPool {
    pub fn new() -> Self {
        let pool_cap = std::env::var("RUST_MYSQL_BUFFER_POOL_CAP")
            .ok()
            .and_then(|x| x.parse().ok())
            .unwrap_or(DEFAULT_MYSQL_BUFFER_POOL_CAP);

        let buffer_cap = std::env::var("RUST_MYSQL_BUFFER_SIZE_CAP")
            .ok()
            .and_then(|x| x.parse().ok())
            .unwrap_or(DEFAULT_MYSQL_BUFFER_SIZE_CAP);

        Self((pool_cap > 0).then(|| {
            Arc::new(Inner {
                buffer_cap,
                pool: ArrayQueue::new(pool_cap),
            })
        }))
    }

    pub fn get(self: &Arc<Self>) -> PooledBuf {
        match self.0 {
            Some(ref inner) => inner.get(),
            None => PooledBuf(Vec::new(), None),
        }
    }
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct PooledBuf(Vec<u8>, Option<Arc<Inner>>);

impl AsMut<Vec<u8>> for PooledBuf {
    fn as_mut(&mut self) -> &mut Vec<u8> {
        &mut self.0
    }
}

impl Deref for PooledBuf {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl Drop for PooledBuf {
    fn drop(&mut self) {
        if let Some(ref inner) = self.1 {
            inner.put(replace(&mut self.0, vec![]));
        }
    }
}
