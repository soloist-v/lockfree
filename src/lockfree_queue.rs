use std::cell::UnsafeCell;
use std::fmt::Formatter;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug)]
pub struct RingBuffer<T, const SIZE: usize> {
    idx_head: AtomicUsize,
    idx_tail: AtomicUsize,
    m_data: [T; SIZE],
}

impl<T: Default, const SIZE: usize> RingBuffer<T, SIZE> {
    #[inline]
    fn new() -> Self {
        RingBuffer::<T, SIZE> {
            idx_head: AtomicUsize::new(0),
            idx_tail: AtomicUsize::new(0),
            m_data: [(); SIZE].map(|_| T::default()),
        }
    }
}

impl<T, const SIZE: usize> RingBuffer<T, SIZE> {
    #[inline]
    fn new_with_init<F>(init: F) -> Self
        where
            F: FnMut(usize) -> T
    {
        let mut init = init;
        let mut i = 0_usize;
        RingBuffer::<T, SIZE> {
            idx_head: AtomicUsize::new(0),
            idx_tail: AtomicUsize::new(0),
            m_data: [(); SIZE].map(|_| {
                let val = init(i);
                i += 1;
                val
            }),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Empty,
    Full,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for Error {}

impl<T, const SIZE: usize> RingBuffer<T, SIZE> {
    fn push(&mut self, value: T) -> Result<(), Error> {
        let mut head = self.idx_head.load(Ordering::Acquire) + 1;
        let tail = self.idx_tail.load(Ordering::Acquire);
        if head == SIZE {
            head = 0;
        }
        if head == tail {
            return Err(Error::Full);
        }
        self.m_data[self.idx_head.load(Ordering::Acquire)] = value;
        self.idx_head.store(head, Ordering::Release);
        Ok(())
    }

    fn pop(&mut self) -> Result<T, Error> {
        let mut tail = self.idx_tail.load(Ordering::Acquire);
        let head = self.idx_head.load(Ordering::Acquire);
        if head == tail {
            return Err(Error::Empty);
        }
        let mut res: T;
        unsafe {
            res = std::mem::zeroed();
        }
        std::mem::swap(&mut res, &mut self.m_data[tail]);
        tail += 1;
        if tail == SIZE {
            tail = 0;
        }
        self.idx_tail.store(tail, Ordering::Release);
        return Ok(res);
    }

    #[inline]
    fn is_full(&self) -> bool {
        let idx_tail = self.idx_tail.load(Ordering::Acquire);
        let idx_head = self.idx_head.load(Ordering::Acquire);
        idx_tail == (idx_head + 1) % SIZE
    }

    #[inline]
    fn is_empty(&self) -> bool {
        let idx_tail = self.idx_tail.load(Ordering::Acquire);
        let idx_head = self.idx_head.load(Ordering::Acquire);
        idx_head == idx_tail
    }

    #[inline]
    pub fn size(&self) -> usize {
        SIZE
    }
}
