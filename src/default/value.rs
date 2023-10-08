use std::ops::{Index, IndexMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use crossbeam_utils::CachePadded;
use super::error::Error;

#[derive(Debug)]
pub struct LockFreeValue<T, const ITEM_SIZE: usize> {
    data: [T; ITEM_SIZE],
    set_idx: CachePadded<AtomicUsize>,
    get_idx: CachePadded<AtomicUsize>,
}

impl<T: Default, const SIZE: usize> LockFreeValue<T, SIZE>
{
    #[inline]
    pub fn new() -> Self {
        Self {
            data: [(); SIZE].map(|_| Default::default()),
            set_idx: CachePadded::new(AtomicUsize::new(0)),
            get_idx: CachePadded::new(AtomicUsize::new(0)),
        }
    }
}

impl<T: Default, const SIZE: usize> LockFreeValue<T, SIZE>
{
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
    pub fn push(&mut self, value: T) -> T {
        let next = self.next_idx_safe();
        let old = self.set_value(next, value);
        self.set_idx.store(next, Ordering::Release);
        old
    }

    /// 设置缓冲区数据
    #[inline]
    pub fn set_value(&mut self, idx: usize, value: T) -> T {
        std::mem::replace(&mut self.data[idx], value)
    }

    /// 设置下一个索引
    #[inline]
    pub fn set_next_idx(&mut self, next_idx: usize) {
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
    pub fn update(&mut self) -> usize {
        self.get_idx.store(self.set_idx.load(Ordering::Acquire), Ordering::Release);
        return self.get_idx.load(Ordering::Acquire);
    }

    /// 获取最新的数据
    #[inline]
    pub fn get_last(&mut self) -> Result<T, Error> {
        let set_idx = self.set_idx.load(Ordering::Acquire);
        let get_idx = self.get_idx.load(Ordering::Acquire);
        if set_idx == get_idx {
            return Err(Error::Empty);
        }
        // 这里注意必须先占坑，这样写入线程就会跳过坑
        self.get_idx.store(set_idx, Ordering::Release);
        let t = std::mem::replace(&mut self.data[set_idx], T::default());
        Ok(t)
    }

    /// 获取最新的数据
    #[inline]
    pub fn get_last_ref(&mut self) -> Result<&T, Error> {
        let set_idx = self.set_idx.load(Ordering::Acquire);
        let get_idx = self.get_idx.load(Ordering::Acquire);
        if set_idx == get_idx {
            return Err(Error::Empty);
        }
        // 这里注意必须先占坑，这样写入线程就会跳过坑
        self.get_idx.store(set_idx, Ordering::Release);
        let t = &self.data[set_idx];
        Ok(t)
    }

    /// 获取最新的数据
    #[inline]
    pub fn get_last_mut(&mut self) -> Result<&mut T, Error> {
        let set_idx = self.set_idx.load(Ordering::Acquire);
        let get_idx = self.get_idx.load(Ordering::Acquire);
        if set_idx == get_idx {
            return Err(Error::Empty);
        }
        // 这里注意必须先占坑，这样写入线程就会跳过坑
        self.get_idx.store(set_idx, Ordering::Release);
        let t = &mut self.data[set_idx];
        Ok(t)
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
    pub fn clear(&mut self) {
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

impl<T: Default, const SIZE: usize> ValueReader<T, SIZE> {
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
    pub fn last_idx(&self) -> usize {
        self.inner.set_idx.load(Ordering::Acquire)
    }

    #[inline]
    pub fn get_last(&mut self) -> Result<T, Error> {
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).get_last()
        }
    }

    #[inline]
    pub fn get_last_ref(&mut self) -> Result<&T, Error> {
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).get_last_ref()
        }
    }

    #[inline]
    pub fn get_last_mut(&mut self) -> Result<&mut T, Error> {
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).get_last_mut()
        }
    }

    #[inline]
    pub fn at(&self, idx: usize) -> &T {
        self.inner.at(idx)
    }
}

impl<T, const SIZE: usize> Index<usize> for ValueReader<T, SIZE> {
    type Output = T;
    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.inner[index]
    }
}

pub struct ValueWriter<T, const SIZE: usize> {
    inner: Arc<LockFreeValue<T, SIZE>>,
}

impl<T: Default, const SIZE: usize> ValueWriter<T, SIZE> {
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
    pub fn push(&mut self, value: T) -> T {
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).push(value)
        }
    }

    /// 设置缓冲区数据
    #[inline]
    pub fn set_value(&mut self, idx: usize, value: T) -> T {
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).set_value(idx, value)
        }
    }

    /// 设置下一个索引，这里使用 mut 限制，如果不限制 意味着 如果被Arc包裹，那么会有多个所有者修改数据，这是不安全的
    #[inline]
    pub fn set_next_idx(&mut self, next_idx: usize) {
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).set_next_idx(next_idx)
        }
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
        unsafe {
            Arc::get_mut_unchecked(&mut self.inner).clear()
        }
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

pub fn make_value<T: Default, const SIZE: usize>() -> (ValueWriter<T, SIZE>, ValueReader<T, SIZE>, )
{
    let ring = Arc::new(LockFreeValue::new());
    let writer = ValueWriter {
        inner: ring.clone(),
    };
    let reader = ValueReader {
        inner: ring,
    };
    (writer, reader)
}
