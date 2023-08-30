use std::ops::{Index, IndexMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub struct LockFreeValue<T, const ITEM_SIZE: usize> {
    data: [T; ITEM_SIZE],
    set_idx: AtomicUsize,
    get_idx: AtomicUsize,
}

impl<T, const SIZE: usize> LockFreeValue<T, SIZE>
    where
        T: Default,
{
    #[inline]
    pub fn new() -> Self {
        LockFreeValue::<T, SIZE> {
            data: [(); SIZE].map(|_| T::default()),
            set_idx: AtomicUsize::new(0),
            get_idx: AtomicUsize::new(0),
        }
    }
}

impl<T, const SIZE: usize> LockFreeValue<T, SIZE>
{
    #[inline]
    pub fn new_with_init<F>(init: F) -> Self
        where
            F: FnMut(usize) -> T,
    {
        let mut init = init;
        let mut i = 0_usize;
        LockFreeValue::<T, SIZE> {
            data: [(); SIZE].map(|_| {
                let t = init(i);
                i += 1;
                t
            }),
            set_idx: AtomicUsize::new(0),
            get_idx: AtomicUsize::new(0),
        }
    }

    /// 缓冲区大小
    #[inline]
    pub fn size(&self) -> usize {
        SIZE
    }
    /// 获取下一个位置的索引
    #[inline]
    pub fn next_idx(&self) -> usize {
        (self.set_idx.load(Ordering::Acquire) + 1) & (SIZE - 1)
    }
    /// 安全地获取下一个位置的索引，这将检查下一个索引是否已经转过一圈
    #[inline]
    pub fn next_idx_safe(&self) -> usize {
        let next = (self.set_idx.load(Ordering::Acquire) + 1) & (SIZE - 1);
        if next == self.get_idx.load(Ordering::Acquire) {
            // 如果下一个位置索引等于当前获取值的索引，则直接跳过获取值索引
            (self.get_idx.load(Ordering::Acquire) + 1) & (SIZE - 1)
        } else {
            next
        }
    }

    /// 放入最新值
    #[inline]
    pub fn push(&mut self, value: T) {
        let next = self.next_idx_safe();
        self.data[next] = value;
        self.set_idx.store(next, Ordering::Release);
    }

    /// 设置缓冲区数据
    #[inline]
    pub fn set_value(&mut self, idx: usize, value: T) {
        self.data[idx] = value;
    }

    /// 设置下一个索引
    #[inline]
    pub fn set_next_idx(&self, next_idx: usize) {
        self.set_idx.store(next_idx, Ordering::Release);
    }

    /// 最新值是否已经发生变化
    #[inline]
    pub fn changed(&self) -> bool {
        return self.get_idx.load(Ordering::Acquire) != self.set_idx.load(Ordering::Acquire);
    }

    /// 最新值是否没有发生变化
    #[inline]
    pub fn unchanged(&self) -> bool {
        self.get_idx.load(Ordering::Acquire) == self.set_idx.load(Ordering::Acquire)
    }

    /// 将获取值的索引更新到最新值的索引
    #[inline]
    pub fn update(&self) -> usize {
        self.get_idx.store(self.set_idx.load(Ordering::Acquire), Ordering::Release);
        return self.get_idx.load(Ordering::Acquire);
    }

    /// 获取最新的数据
    #[inline]
    pub fn get_last(&self) -> &T {
        let set_idx = self.set_idx.load(Ordering::Acquire);
        self.get_idx.store(set_idx, Ordering::Release);
        return &self.data[set_idx];
    }

    /// 获取最新值，如果未更新返回None
    #[inline]
    pub fn get_next(&self) -> Option<&T> {
        if self.unchanged() {
            return None;
        }
        return Some(self.get_last());
    }

    /// 获取缓冲区数据
    #[inline]
    pub fn at(&self, idx: usize) -> &T {
        &self.data[idx]
    }


    /// 获取缓冲区数据可变
    #[inline]
    pub fn at_mut(&mut self, idx: usize) -> &mut T {
        &mut self.data[idx]
    }

    /// 清除整个缓冲区
    #[inline]
    pub fn clear(&self) {
        self.set_idx.store(0, Ordering::Release);
        self.get_idx.store(0, Ordering::Release);
    }
}

impl<T, const S: usize> Index<usize> for LockFreeValue<T, S> {
    type Output = T;
    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl<T, const S: usize> IndexMut<usize> for LockFreeValue<T, S> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data[index]
    }
}

pub struct ValueReader<T, const SIZE: usize> {
    inner: Arc<LockFreeValue<T, SIZE>>,
}

impl<T, const SIZE: usize> ValueReader<T, SIZE> {
    /// 缓冲区大小
    #[inline]
    pub fn size(&self) -> usize {
        self.inner.size()
    }

    /// 最新值是否已经发生变化
    #[inline]
    pub fn changed(&self) -> bool {
        self.inner.changed()
    }

    /// 最新值是否没有发生变化
    #[inline]
    pub fn unchanged(&self) -> bool {
        self.inner.unchanged()
    }

    #[inline]
    pub fn get_last(&self) -> &T {
        self.inner.get_last()
    }

    #[inline]
    pub fn get_next(&self) -> Option<&T> {
        self.inner.get_next()
    }

    #[inline]
    pub fn at(&self, idx: usize) -> &T {
        self.inner.at(idx)
    }
}

impl<T, const SIZE: usize> Index<usize> for ValueReader<T, SIZE> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.inner[index]
    }
}

pub struct ValueWriter<T, const SIZE: usize> {
    inner: Arc<LockFreeValue<T, SIZE>>,
}

impl<T, const SIZE: usize> ValueWriter<T, SIZE> {
    /// 缓冲区大小
    #[inline]
    pub fn size(&self) -> usize {
        self.inner.size()
    }
    /// 获取下一个位置的索引
    #[inline]
    pub fn next_idx(&self) -> usize {
        self.inner.next_idx()
    }
    /// 安全地获取下一个位置的索引，这将检查下一个索引是否已经转过一圈
    #[inline]
    pub fn next_idx_safe(&self) -> usize {
        self.inner.next_idx_safe()
    }

    /// 放入最新值
    #[inline]
    pub fn push(&mut self, value: T) {
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).push(value)
        }
    }

    /// 设置缓冲区数据
    #[inline]
    pub fn set_value(&mut self, idx: usize, value: T) {
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).set_value(idx, value)
        }
    }

    /// 设置下一个索引，这里使用 mut 限制，如果不限制 意味着 如果被Arc包裹，那么会有多个所有者修改数据，这是不安全的
    #[inline]
    pub fn set_next_idx(&mut self, next_idx: usize) {
        self.inner.set_next_idx(next_idx)
    }

    /// 最新值是否已经发生变化
    #[inline]
    pub fn changed(&self) -> bool {
        self.inner.changed()
    }

    /// 最新值是否没有发生变化
    #[inline]
    pub fn unchanged(&self) -> bool {
        self.inner.unchanged()
    }

    /// 获取缓冲区数据
    #[inline]
    pub fn at(&self, idx: usize) -> &T {
        self.inner.at(idx)
    }

    /// 获取缓冲区数据可变
    #[inline]
    pub fn at_mut(&mut self, idx: usize) -> &mut T {
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).at_mut(idx)
        }
    }

    /// 清除整个缓冲区 这里使用 mut 限制，如果不限制 意味着 如果被Arc包裹，那么会有多个所有者修改数据，这是不安全的
    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }
}

impl<T, const SIZE: usize> Index<usize> for ValueWriter<T, SIZE> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.inner[index]
    }
}

impl<T, const S: usize> IndexMut<usize> for ValueWriter<T, S> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe {
            &mut Arc::get_mut_unchecked(&mut self.inner)[index]
        }
    }
}

pub fn make_value<T: Default, const SIZE: usize>() -> (ValueReader<T, SIZE>, ValueWriter<T, SIZE>)
{
    let ring = Arc::new(LockFreeValue::new());
    let reader = ValueReader {
        inner: ring.clone(),
    };
    let writer = ValueWriter {
        inner: ring,
    };
    (reader, writer)
}

pub fn make_value_init<T: Default, const SIZE: usize>(
    init: impl FnMut(usize) -> T) -> (ValueReader<T, SIZE>, ValueWriter<T, SIZE>)
{
    let ring = Arc::new(LockFreeValue::new_with_init(init));
    let reader = ValueReader {
        inner: ring.clone(),
    };
    let writer = ValueWriter {
        inner: ring,
    };
    (reader, writer)
}