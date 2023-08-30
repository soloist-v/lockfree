use crate::as_mut;
use std::path::Display;
use std::sync::Arc;

#[derive(Debug)]
pub struct RingBuffer<T, const M_SIZE: usize> {
    idx_head: usize,
    idx_tail: usize,
    m_data: [T; M_SIZE],
}

pub trait Queue<T> {
    fn new_empty() -> Self;
    fn push(&mut self, value: T) -> bool;
    fn pop(&mut self) -> Option<T>;
    fn is_full(&self) -> bool;
    fn is_empty(&self) -> bool;
}

pub trait Init<T, F: FnMut(usize) -> T> {
    fn new(initializer: F) -> Self;
}

impl<T, const SIZE: usize, F: FnMut(usize) -> T> Init<T, F> for RingBuffer<T, SIZE> {
    fn new(initializer: F) -> Self {
        RingBuffer::<T, SIZE> {
            idx_head: 0,
            idx_tail: 0,
            m_data: array_init::array_init(initializer),
        }
    }
}

impl<T, const SIZE: usize> Queue<T> for RingBuffer<T, SIZE> {
    fn new_empty() -> Self {
        RingBuffer::<T, SIZE> {
            idx_head: 0,
            idx_tail: 0,
            m_data: array_init::array_init(|_| unsafe { std::mem::zeroed() }),
        }
    }

    // fn push(&mut self, value: T) -> bool {
    //     if self.is_full() {
    //         return false;
    //     }
    //     // log::info!("push: {:p}, {:p}", &self.idx_head, self);
    //     self.m_data[self.idx_tail] = value;
    //     self.idx_tail = (self.idx_tail + 1) % Size;
    //     return true;
    // }

    fn push(&mut self, value: T) -> bool {
        let mut head = unsafe { std::ptr::read_volatile(&self.idx_head) + 1 };
        let tail = unsafe { std::ptr::read_volatile(&self.idx_tail) };
        if head == SIZE {
            head = 0;
        }
        if head == tail {
            return false;
        }
        self.m_data[self.idx_head] = value;
        unsafe {
            std::ptr::write_volatile(&mut self.idx_head, head);
        }
        return true;
    }

    fn pop(&mut self) -> Option<T> {
        let mut tail = unsafe { std::ptr::read_volatile(&self.idx_tail) };
        let head = unsafe { std::ptr::read_volatile(&self.idx_head) };
        if head == tail {
            return None;
        }
        // let res = &self.m_data[tail];
        let mut res: T;
        unsafe {
            res = std::mem::zeroed();
        }
        // std::mem::replace()
        std::mem::swap(&mut res, &mut self.m_data[tail]);
        tail += 1;
        if tail == SIZE {
            tail = 0;
        }
        unsafe {
            std::ptr::write_volatile(&mut self.idx_tail, tail);
        }
        return Some(res);
    }

    // fn pop(&mut self) -> Option<&T> {
    //     if self.is_empty() {
    //         return None;
    //     }
    //     // log::info!("push: {:p}, {:p}", &self.idx_head, self);
    //     let val = &self.m_data[self.idx_head];
    //     self.idx_head = (self.idx_head + 1) % Size;
    //     return Some(val);
    // }

    // fn is_full(&self) -> bool {
    //     // let idx_head = &self.idx_head as *const usize;
    //     // let idx_tail = &self.idx_tail as *const usize;
    //     // let res = unsafe {
    //     //     std::ptr::read_volatile(idx_head) == (std::ptr::read_volatile(idx_tail) + 1) % Size
    //     // };
    //     // log::info!("is_full: {}", res);
    //     // return res;
    //     self.idx_head == (self.idx_tail + 1) % Size
    // }

    fn is_full(&self) -> bool {
        self.idx_tail == (self.idx_head + 1) % SIZE
    }

    fn is_empty(&self) -> bool {
        // let idx_head = &self.idx_head as *const usize;
        // let idx_tail = &self.idx_tail as *const usize;
        // let res = unsafe {
        //     std::ptr::read_volatile(idx_head) == std::ptr::read_volatile(idx_tail)
        // };
        // log::info!("is_empty: {}", res);
        // return res;
        self.idx_head == self.idx_tail
    }
}
#[allow(dead_code)]
impl<T, const SIZE: usize> RingBuffer<T, SIZE> {
    pub fn clear(&mut self) {
        self.m_data = array_init::array_init(|_| unsafe { std::mem::zeroed() });
        self.idx_head = 0;
        self.idx_tail = 0;
    }
    pub fn front(&self) -> usize {
        self.idx_head
    }
    pub fn rear(&self) -> usize {
        self.idx_tail
    }
    pub fn size() -> usize {
        SIZE
    }
}
