use super::spin_lock::{Spinlock, SpinlockGuard};
use libc::sbrk;
use std::cmp;
use std::{
    alloc::{GlobalAlloc, Layout},
    mem::{self, align_of, size_of},
};
use super::utils;


pub struct SbrkAllocator {
    free_list: FreeBlockList,
}

pub struct Locked<T> {
    inner: Spinlock<T>,
}

// Problem: MacOs Mutexes use pthreads, which are Box allocated!
// So if we use std::sync::Mutex we create a loop here: The allocator's Mutex would require a Box which requires the allocator to work!
// So we implement a custom stack based lock to avoid that
impl<T> Locked<T> {
    pub const fn new(inner: T) -> Self {
        Locked {
            inner: Spinlock::new(inner),
        }
    }

    pub fn lock(&self) -> SpinlockGuard<T> {
        self.inner.lock()
    }
}

// rust doesn't allow implementing traits for external types. This maintains a property called coherence
// So we must wrap our allocator in a Locked type ourselves
unsafe impl GlobalAlloc for Locked<SbrkAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut l = self.lock();
        let (size, align) = l.align_layout(layout);
        l.malloc(size, align)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut l = self.lock();
        let (size, _) = l.align_layout(layout);
        l.free(ptr, size)
    }
}

impl SbrkAllocator {
    pub const fn new() -> Self {
        Self {
            free_list: FreeBlockList::new(),
        }
    }

    /// Aligns the input layout in order to make sure we can allocate
    /// a FreeBlock on the new memory region
    fn align_layout(&self, layout: Layout) -> (usize, usize) {
        let max_align = cmp::max( align_of::<FreeBlock>(), layout.align());
        let out_size = utils::to_align(layout.size() + size_of::<FreeBlock>(), max_align);
        (out_size, max_align)
    }

    unsafe fn request_sys_mem(size: i32) -> *mut u8 {
        let ptr = sbrk(size) as isize;
        assert_ne!(ptr, -1); // sbrk returns pointer to -1 if it fails
        return ptr as *mut u8;
    }

    unsafe fn malloc(&mut self, size: usize, align: usize) -> *mut u8 {
        match self.free_list.find_free_block(size, align) {
            Some((blk, addr)) => {
                let end = blk.start_addr().checked_add(size).expect("overflow error");
                let excess = blk.end_addr() - end;
                if excess > 0 {
                    self.free_list.add_free_block(end as *mut u8, excess); // If the found blk is larger than we need, allocate the rest of it as a FreeBlock
                }
                addr as *mut u8
            }
            None => {
                Self::request_sys_mem(size as i32)
            }
        }
    }
    pub unsafe fn free(&mut self, ptr: *mut u8, size: usize) {
        self.free_list.add_free_block(ptr, size);
    }
}

pub struct FreeBlock {
    size: usize,
    // to avoid unstable features we might need to use pointers
    next: Option<&'static mut FreeBlock>,
}

pub struct FreeBlockList {
    head: FreeBlock,
}

impl FreeBlockList {
    const fn new() -> Self {
        Self {
            head: FreeBlock::new(0),
        }
    }

    /// Push operation. Adds a new FreeBlock node to the list
    unsafe fn add_free_block(&mut self, ptr: *mut u8, size: usize) {
        let mut node = FreeBlock::new(size);
        // takes value out of option
        node.next = self.head.next.take();
        let node_ptr = ptr as *mut FreeBlock;
        node_ptr.write(node); // write the object data to the pointer, so we avoid lifetime issues as the stack's node will be dropped here
        self.head.next = Some(&mut *node_ptr); // puts new value back to tail next option
    }

    fn find_free_block(
        &mut self,
        size: usize,
        align: usize,
    ) -> Option<(&'static mut FreeBlock, usize)> {
        let mut current = &mut self.head;

        while let Some(ref mut blk) = current.next {
            if let Ok(start_addr) = Self::alloc_from_block(blk, size, align) {
                let next = blk.next.take();
                let ret = Some((current.next.take().unwrap(), start_addr));
                current.next = next;
                return ret;
            }
            current = current.next.as_mut().unwrap();
        }
        None // if we reach the end of list, no more free memory
    }

    fn alloc_from_block(block: &FreeBlock, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = utils::to_align(block.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > block.end_addr() {
            // region too small
            return Err(());
        }

        // size for Freeblock
        let excess_size = block.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<FreeBlock>() {
            // rest of region too small to hold a FreeBlock
            // if the current FreeBlock is too large, we can allocate only `size`
            // and then allocate a FreeBlock for the rest of it so we use the resource well
            return Err(());
        }

        Ok(alloc_start)
    }

}

impl FreeBlock {
    const fn new(size: usize) -> Self {
        FreeBlock { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

// 44 bytes
#[derive(Debug)]
pub struct Thunk {
    field1: u32,
    field2: [u32; 10],
}
// 4 bytes
#[derive(Debug)]
pub struct smallerThunk {
    field1: u32,
}

#[cfg(test)]
mod tests {
    // use std::{mem::align_of, mem::size_of, ptr::NonNull};
    use libc::sbrk;
    use super::{Thunk, SbrkAllocator};
    // use super::{SbrkAllocator, FreeBlock, FreeBlockList, Test};
    // use crate::cbindings::sbrk::Thunk;

    #[test]
    fn test_malloc() {
        let mut alloc = SbrkAllocator::new();

        let ref_t = unsafe {
            let test_t = alloc.malloc(size_of::<Thunk>(), align_of::<Thunk>()) as *mut Thunk;
            &*test_t
        };

        assert_eq!(size_of_val(ref_t), size_of::<Thunk>());
        assert_eq!(align_of_val(ref_t), align_of::<Thunk>());
    }

    // Test to show memory reuse. After freeing the first pointer, we ask for another object. 
    // The free-list then returns a free block for us
    #[test]
    fn test_free_reuse() {
        unsafe {
            let mut alloc = SbrkAllocator::new();

            let p = alloc.malloc(size_of::<Thunk>(), align_of::<Thunk>()) as *mut Thunk;

            let p2 = alloc.malloc(size_of::<Thunk>(), align_of::<Thunk>()) as *mut Thunk;

            alloc.free(p as *mut u8, size_of::<Thunk>());

            let p3 = alloc.malloc(size_of::<Thunk>(), align_of::<Thunk>()) as *mut Thunk;
            assert_eq!(p, p3);
        }
    }
}
