use std::fmt::Formatter;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 这里其实不需要限制RingBuffer，因为RingBuffer的实现都是符合借用规则的
/// 所以不必担心安全问题，默认情况下只会有一个线程持有对象，因为没有提供Clone方法，即便用Arc指针
/// 也无法通过不可变引用修改内部数据
/// 如果想要修改内部数据就必须在包一层Mutex，这也是完全符合安全原则的
/// 因此如果想要使用就必须使用unsafe，此时安全由使用者确保
/// 所以在下面的读写分离实现中，使用了Arcell实现内部可变。
#[derive(Debug)]
pub struct RingBuffer<T, const SIZE: usize = 4> {
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

/// 这里采用Reader 和Writer的分离实现
/// 由于Reader没有实现Clone，所以Reader不能共享所有权
/// 由于Writer没事实现Clone，所以Writer不能共享所有权
/// 因此，就实现了 单生产者-单消费者 模式

pub struct RingBufferReader<T, const SIZE: usize> {
    inner: Arc<RingBuffer<T, SIZE>>,
}

impl<T, const SIZE: usize> RingBufferReader<T, SIZE> {
    fn push(&mut self, value: T) -> Result<(), Error> {
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).push(value)
        }
    }
}

pub struct RingBufferWriter<T, const SIZE: usize> {
    inner: Arc<RingBuffer<T, SIZE>>,
}

impl<T, const SIZE: usize> RingBufferWriter<T, SIZE> {
    fn pop(&mut self) -> Result<T, Error> {
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).pop()
        }
    }
}

pub fn ringbuffer<T: Default, const SIZE: usize>() -> (RingBufferReader<T, SIZE>, RingBufferWriter<T, SIZE>)
{
    let ring = Arc::new(RingBuffer::new());
    let reader = RingBufferReader {
        inner: ring.clone(),
    };
    let writer = RingBufferWriter {
        inner: ring,
    };
    (reader, writer)
}

pub fn ringbuffer_init<T: Default, const SIZE: usize>(
    init: impl FnMut(usize) -> T) -> (RingBufferReader<T, SIZE>, RingBufferWriter<T, SIZE>)
{
    let ring = Arc::new(RingBuffer::new_with_init(init));
    let reader = RingBufferReader {
        inner: ring.clone(),
    };
    let writer = RingBufferWriter {
        inner: ring,
    };
    (reader, writer)
}