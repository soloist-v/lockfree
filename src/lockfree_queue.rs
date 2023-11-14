#![allow(dead_code)]

use std::fmt::Formatter;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use crossbeam_utils::CachePadded;

/// 这里其实不需要限制RingBuffer，因为RingBuffer的实现都是符合借用规则的
/// 所以不必担心安全问题，默认情况下只会有一个线程持有对象，因为没有提供Clone方法，即便用Arc指针
/// 也无法通过不可变引用修改内部数据
/// 如果想要修改内部数据就必须在包一层Mutex，这也是完全符合安全原则的
/// 因此如果想要使用就必须使用unsafe，此时安全由使用者确保
/// 所以在下面的读写分离实现中，使用了Arc实现内部可变。
#[derive(Debug)]
pub struct RingBuffer<T, const SIZE: usize = 4> {
    m_data: [Option<T>; SIZE],
    idx_head: CachePadded<AtomicUsize>,
    idx_tail: CachePadded<AtomicUsize>,
}

impl<T, const SIZE: usize> RingBuffer<T, SIZE> {
    #[inline]
    fn new() -> Self {
        RingBuffer::<T, SIZE> {
            idx_head: CachePadded::new(AtomicUsize::new(0)),
            idx_tail: CachePadded::new(AtomicUsize::new(0)),
            m_data: [(); SIZE].map(|_| None),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Empty,
    Full,
    InterDisordered,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for Error {}


impl<T, const SIZE: usize> RingBuffer<T, SIZE> {
    #[inline]
    fn next_idx(cur: usize) -> usize {
        (cur + 1) & (SIZE - 1)
    }

    #[inline]
    fn ring_idx(cur: usize) -> usize {
        cur & (SIZE - 1)
    }

    pub fn push(&mut self, value: T) -> Result<(), Error> {
        let head = self.idx_head.load(Ordering::Acquire);
        let tail = self.idx_tail.load(Ordering::Acquire);
        let mut next_head = Self::next_idx(head);
        if next_head == tail {
            return Err(Error::Full);
        }
        self.m_data[head].replace(value);
        self.idx_head.store(next_head, Ordering::Release);
        Ok(())
    }

    pub fn pop(&mut self) -> Result<T, Error> {
        let mut tail = self.idx_tail.load(Ordering::Acquire);
        let head = self.idx_head.load(Ordering::Acquire);
        if head == tail {
            return Err(Error::Empty);
        }
        let res = self.m_data[tail].take();
        self.idx_tail.store(Self::next_idx(tail), Ordering::Release);
        match res {
            None => {
                Err(Error::InterDisordered)
            }
            Some(a) => {
                Ok(a)
            }
        }
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        let idx_tail = self.idx_tail.load(Ordering::Acquire);
        let idx_head = self.idx_head.load(Ordering::Acquire);
        idx_tail == Self::next_idx(idx_head)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        let idx_tail = self.idx_tail.load(Ordering::Acquire);
        let idx_head = self.idx_head.load(Ordering::Acquire);
        idx_head == idx_tail
    }

    #[inline]
    pub fn size(&self) -> usize {
        SIZE
    }
}

/// 这里采用Reader 和Writer的分离实现
/// 由于Reader没有实现Clone，所以Reader不能共享所有权
/// 由于Writer没有实现Clone，所以Writer不能共享所有权
/// 因此，就实现了 单生产者-单消费者 模式

pub struct RingBufferSender<T, const SIZE: usize> {
    inner: Arc<RingBuffer<T, SIZE>>,
}

impl<T, const SIZE: usize> RingBufferSender<T, SIZE> {
    #[inline]
    fn is_full(&self) -> bool {
        self.inner.is_full()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.inner.size()
    }
    fn push(&mut self, value: T) -> Result<(), Error> {
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).push(value)
        }
    }
}

pub struct RingBufferReceiver<T, const SIZE: usize> {
    inner: Arc<RingBuffer<T, SIZE>>,
}

impl<T, const SIZE: usize> RingBufferReceiver<T, SIZE> {
    #[inline]
    fn is_full(&self) -> bool {
        self.inner.is_full()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.inner.size()
    }
    fn pop(&mut self) -> Result<T, Error> {
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).pop()
        }
    }
}

pub fn ringbuffer<T, const SIZE: usize>() -> (RingBufferSender<T, SIZE>, RingBufferReceiver<T, SIZE>)
{
    let ring = Arc::new(RingBuffer::new());
    let sender = RingBufferSender {
        inner: ring.clone(),
    };
    let receiver = RingBufferReceiver {
        inner: ring,
    };
    (sender, receiver)
}
