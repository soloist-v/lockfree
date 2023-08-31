use std::fmt::Formatter;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use crossbeam_utils::CachePadded;

/// 这里其实不需要限制RingBuffer，因为RingBuffer的实现都是符合借用规则的
/// 所以不必担心安全问题，默认情况下只会有一个线程持有对象，因为没有提供Clone方法，即便用Arc指针
/// 也无法通过不可变引用修改内部数据
/// 如果想要修改内部数据就必须在包一层Mutex，这也是完全符合安全原则的
/// 因此如果想要使用就必须使用unsafe，此时安全由使用者确保
/// 所以在下面的读写分离实现中，使用了Arc实现内部可变，由于要约束mut，所以Arc的get_mut_unchecked也可以使用。
/// 由于值要被取走，转移所有权，因此 m_data 只能使用MaybeUninit<T>，因为如果直接使用T，T被替换的时候不一定支持Default
#[derive(Debug)]
pub struct RingBuffer<T, const SIZE: usize = 4> {
    idx_head: CachePadded<AtomicUsize>,
    idx_tail: CachePadded<AtomicUsize>,
    m_data: [MaybeUninit<T>; SIZE],
}

impl<T, const SIZE: usize> RingBuffer<T, SIZE> {
    #[inline]
    fn new() -> Self {
        RingBuffer::<T, SIZE> {
            idx_head: CachePadded::new(AtomicUsize::new(0)),
            idx_tail: CachePadded::new(AtomicUsize::new(0)),
            m_data: [(); SIZE].map(|_| MaybeUninit::uninit()),
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
        unsafe {
            let o = &mut self.m_data[self.idx_head.load(Ordering::Acquire)];
            // 先读取
            let _t = o.assume_init_read();
            // 再写入
            o.write(value);
        }
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

pub struct RingBufferSender<T, const SIZE: usize> {
    inner: Arc<RingBuffer<T, SIZE>>,
}

impl<T, const SIZE: usize> RingBufferSender<T, SIZE> {
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
    fn pop(&mut self) -> Result<T, Error> {
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).pop()
        }
    }
}

pub fn ringbuffer<T: Default, const SIZE: usize>() -> (RingBufferSender<T, SIZE>, RingBufferReceiver<T, SIZE>)
{
    let ring = Arc::new(RingBuffer::new());
    let reader = RingBufferSender {
        inner: ring.clone(),
    };
    let writer = RingBufferReceiver {
        inner: ring,
    };
    (reader, writer)
}

pub fn ringbuffer_init<T: Default, const SIZE: usize>(
    init: impl FnMut(usize) -> T) -> (RingBufferSender<T, SIZE>, RingBufferReceiver<T, SIZE>)
{
    let ring = Arc::new(RingBuffer::new_with_init(init));
    let reader = RingBufferSender {
        inner: ring.clone(),
    };
    let writer = RingBufferReceiver {
        inner: ring,
    };
    (reader, writer)
}