// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct ObjectPool<T> {
    pool: std::sync::Mutex<VecDeque<T>>,
    in_use: AtomicUsize,
    max_size: usize,
    factory: Box<dyn Fn() -> T + Send + Sync>,
}

impl<T: 'static> ObjectPool<T> {
    pub fn new<F>(max_size: usize, factory: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            pool: std::sync::Mutex::new(VecDeque::with_capacity(max_size)),
            in_use: AtomicUsize::new(0),
            max_size,
            factory: Box::new(factory),
        }
    }

    pub fn get(&self) -> PooledObject<'_, T> {
        let obj = {
            let mut pool = self.pool.lock().unwrap();
            pool.pop_front()
        };

        let value = obj.unwrap_or_else(|| (self.factory)());
        self.in_use.fetch_add(1, Ordering::Relaxed);

        PooledObject {
            pool: self,
            value: Some(value),
        }
    }

    pub fn return_object(&self, _value: T) {
        let mut pool = self.pool.lock().unwrap();

        if pool.len() < self.max_size {
            pool.push_back(_value);
        }

        self.in_use.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn in_use_count(&self) -> usize {
        self.in_use.load(Ordering::Relaxed)
    }

    pub fn available_count(&self) -> usize {
        let pool = self.pool.lock().unwrap();
        pool.len()
    }

    pub fn total_count(&self) -> usize {
        self.in_use.load(Ordering::Relaxed) + self.available_count()
    }

    pub fn clear(&self) {
        let mut pool = self.pool.lock().unwrap();
        pool.clear();
    }

    pub fn prefill(&self, count: usize) {
        let mut pool = self.pool.lock().unwrap();

        for _ in 0..count {
            if pool.len() < self.max_size {
                pool.push_back((self.factory)());
            }
        }
    }
}

pub struct PooledObject<'a, T: 'static> {
    pool: &'a ObjectPool<T>,
    value: Option<T>,
}

impl<'a, T> PooledObject<'a, T>
where
    T: 'static,
{
    pub fn get(&mut self) -> &mut T {
        self.value.as_mut().unwrap()
    }

    pub fn take(mut self) -> T {
        self.value.take().unwrap()
    }
}

impl<'a, T> Drop for PooledObject<'a, T>
where
    T: 'static,
{
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            self.pool.return_object(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_pool_basic() {
        let pool = ObjectPool::new(5, || vec![0; 10]);

        let mut obj = pool.get();
        obj.get().push(42);

        assert_eq!(pool.in_use_count(), 1);
        assert_eq!(pool.available_count(), 0);

        drop(obj);

        assert_eq!(pool.in_use_count(), 0);
        assert_eq!(pool.available_count(), 1);
    }

    #[test]
    fn test_object_pool_exhausted() {
        let pool = ObjectPool::new(2, || String::new());

        let _obj1 = pool.get();
        let _obj2 = pool.get();
        let _obj3 = pool.get();

        assert_eq!(pool.in_use_count(), 3);
        assert_eq!(pool.available_count(), 0);
    }

    #[test]
    fn test_pooled_object_take() {
        let pool = ObjectPool::new(5, || 42);

        let obj = pool.get();
        let value = obj.take();

        assert_eq!(value, 42);
        assert_eq!(pool.in_use_count(), 1);
    }

    #[test]
    fn test_object_pool_prefill() {
        let pool = ObjectPool::new(10, || String::from("prefilled"));

        pool.prefill(5);

        assert_eq!(pool.available_count(), 5);
    }
}
