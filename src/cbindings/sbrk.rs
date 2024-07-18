use super::spin_lock::{Spinlock, SpinlockGuard};
use super::utils;
use libc::sbrk;
use std::cmp;
use std::{
    alloc::{GlobalAlloc, Layout},
    mem::{self, align_of, size_of},
};

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
        let max_align = cmp::max(align_of::<FreeBlock>(), layout.align());
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
            None => Self::request_sys_mem(size as i32),
        }
    }
    pub unsafe fn free(&mut self, ptr: *mut u8, size: usize) {
        self.free_list.add_free_block(ptr, size);
    }
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
        self.head.next = Some(&mut *node_ptr); // puts new value in the head of the list
    }

    fn find_free_block(
        &mut self,
        size: usize,
        align: usize,
    ) -> Option<(&'static mut FreeBlock, usize)> {
        let mut current = &mut self.head;

        while let Some(ref mut blk) = current.next {
            if let Ok(start_addr) = Self::check_block(blk, size, align) {
                let next = blk.next.take();
                let ret = Some((current.next.take().unwrap(), start_addr));
                current.next = next;
                return ret;
            }
            current = current.next.as_mut().unwrap();
        }
        None // if we reach the end of list, no more free memory
    }

    /// Checks if block is suitable for allocation of size `size`
    fn check_block(block: &FreeBlock, size: usize, align: usize) -> Result<usize, ()> {
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

pub struct FreeBlock {
    size: usize,
    next: Option<&'static mut FreeBlock>,
}

impl FreeBlock {
    const fn new(size: usize) -> Self {
        FreeBlock { size, next: None } // needs #![feature(const_mut_refs)] -> unstable
    }

    // does this work because the heap starts on the end of the stack?
    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

mod tests {

    pub struct Thunk {
        field1: u32,
        field2: [u32; 10],
    }
    // 4 bytes
    #[derive(Debug)]
    pub struct SmallThunk {
        field1: u32,
    }

    #[cfg(feature = "use_sbrk_allocator")]
    #[cfg(test)]
    mod tests_allocator {
        use std::alloc::{alloc, Layout};
        use super::{Thunk, SmallThunk};
        #[test]
        fn test_alloc_base() {
            // this doesn't work if SbrkAllocator itself is the global allocator. We might mess with it's data
            // let mut alloc = SbrkAllocator::new();
    
            let size = size_of::<Thunk>();
            let align = align_of::<Thunk>();
    
            if let Ok(layout) = Layout::from_size_align(size, align) {
                let ref_t = unsafe {
                    let test_t = alloc(layout) as *mut Thunk;
                    &*test_t
                };
                assert_eq!(size_of_val(ref_t), size_of::<Thunk>());
                assert_eq!(align_of_val(ref_t), align_of::<Thunk>());
                return;
            }
            panic!("Couldn't get layout");
        }
    }
    
    #[cfg(not(feature = "use_sbrk_allocator"))]
    /// We can't run tests in the internal SbrkAllocator if we set the global allocator to it, as we'd have two
    /// independent allocators managing the heap via SBRK, which breaks the allocator. So the `use_sbrk_allocator` feature selects the tests that can run
    /// when the global allocator isn't set.
    #[cfg(test)]
    mod test_internals {
        use super::super::SbrkAllocator;
        use super::{Thunk, SmallThunk};
    
        #[test]
        fn test_malloc_excess() {
            let mut alloc = SbrkAllocator::new();
            unsafe {
                let large_value_addr = alloc.malloc(size_of::<Thunk>(), align_of::<Thunk>());
                alloc.free(large_value_addr, size_of::<Thunk>());
    
                // Allocates object that is smaller than Thunk. So the FreeBlock will have excess space that must remain free
                let _small_value_addr = alloc.malloc(size_of::<SmallThunk>(), align_of::<SmallThunk>());
    
                assert!(alloc.free_list.head.next.is_some_and(|b| {
                    // The first FreeBlock should start in the excess space of the second allocation
                    let excess_offset = large_value_addr as u8 + size_of::<SmallThunk>() as u8;
                    b.start_addr() as u8 == excess_offset
                }));
            };
        }
    
        // Test to show memory reuse. After freeing the first pointer, we ask for another object.
        // The free-list then returns a free-block corresponding to the first (now freed) allocation
        #[test]
        fn test_free_reuse() {
            unsafe {
                let mut alloc = SbrkAllocator::new();
    
                let first_ptr = alloc.malloc(size_of::<Thunk>(), align_of::<Thunk>()) as *mut Thunk;
    
                let _second_ptr = alloc.malloc(size_of::<Thunk>(), align_of::<Thunk>()) as *mut Thunk;
    
                alloc.free(first_ptr as *mut u8, size_of::<Thunk>());
    
                let third_ptr = alloc.malloc(size_of::<Thunk>(), align_of::<Thunk>()) as *mut Thunk;
                assert_eq!(first_ptr, third_ptr);
            }
        }
    }
}


