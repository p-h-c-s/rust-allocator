use super::spin_lock::{Spinlock, SpinlockGuard};
use libc::sbrk;
use std::sync::{LockResult, Mutex};
use std::{
    alloc::{GlobalAlloc, Layout},
    mem::{self, align_of, size_of},
    ptr::{self, NonNull},
    sync::MutexGuard,
};

pub struct Allocator {
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
unsafe impl GlobalAlloc for Locked<Allocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let l = &mut self.lock();
        l.malloc(layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let l = &mut self.lock();
        l.free(ptr, layout.size(), layout.align())
    }
}

impl Allocator {
    pub const fn new() -> Self {
        Self {
            free_list: FreeBlockList::new(),
        }
    }
    unsafe fn malloc(&mut self, size: usize, align: usize) -> *mut u8 {
        // block already exists
        match self.free_list.find_free_block(size, align) {
            Some((_, addr)) => {
                // reuse
                let obj = addr as *mut u8;
                return obj;
            }
            None => {
                let block_size = size_of::<FreeBlock>() as usize;
                let total_size = block_size + size;

                let incr = total_size + (total_size % mem::align_of::<FreeBlock>());
                let ptr = sbrk(incr as i32);
                return ptr as *mut u8;
            }
        }
    }
    pub unsafe fn free(&mut self, ptr: *mut u8, size: usize, align: usize) {
        self.free_list.add_free_region(ptr, size);
    }
}

/// The idea is to create a linked list of FREE nodes
/// The start of the block memory represents the start of allocatable memory
/// So *const FreeBlock returns a pointer to the start of the allocatable memory
// #[repr(C)]
pub struct FreeBlock {
    size: usize,
    // to avoid unstable features we might need to use pointers
    next: Option<&'static mut FreeBlock>,
}

// x bytes alocaveis
// allocate 44 bytes allign 4 = 44 bytes
//
pub struct FreeBlockList {
    head: FreeBlock,
}

impl FreeBlockList {
    const fn new() -> Self {
        Self {
            head: FreeBlock::new(0),
        }
    }

    // free adds a node to the free-list. It's a push operation
    unsafe fn add_free_region(&mut self, ptr: *mut u8, size: usize) {
        let mut node = FreeBlock::new(size);
        // takes value out of option
        node.next = self.head.next.take();
        let node_ptr = ptr as *mut FreeBlock;
        node_ptr.write(node); // write the object data to the pointer, so we avoid lifetime issues
        self.head.next = Some(&mut *node_ptr); // puts new value back to tail next option
    }

    fn find_free_block(
        &mut self,
        size: usize,
        align: usize,
    ) -> Option<(&'static mut FreeBlock, usize)> {
        let mut current = &mut self.head;

        // ref allows the pattern matching to only borrow instead of moving
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
        // does this cause memory leaks? -> unused memory between start_addr and align_up
        let alloc_start = Self::align_up(block.start_addr(), align);
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

    // let allignment = total_size + (total_size % mem::align_of::<Self>());
    fn align_up(addr: usize, align: usize) -> usize {
        (addr + align - 1) & !(align - 1)
    }
}

/// Allocates total_size = data_size + block_size bytes
/// Returns a pointer to the start of 0..total_size
/// const_mut_refs is unstable, how can we avoid the mutable ref in const here?
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

// maybe use https://stackoverflow.com/questions/56613171/does-casting-pointers-in-rust-have-the-same-behavior-as-reinterpret-cast-in-c
#[derive(Debug)]
pub struct Test {
    field1: u32,
    field2: [u32; 10],
}

#[derive(Debug)]
pub struct Thunk {
    field2: [u32; 13],
}

#[cfg(test)]
mod tests {
    use std::{mem::align_of, mem::size_of, ptr::NonNull};

    use libc::sbrk;

    use crate::cbindings::sbrk::Thunk;

    use super::{Allocator, FreeBlock, FreeBlockList, Test};

    #[test]
    fn test_malloc() {
        unsafe {
            let curr_heap_start = sbrk(0);
            let mut alloc = Allocator::new();

            let p = alloc.malloc(size_of::<Test>(), align_of::<Test>()) as *mut Test;

            let p2 = alloc.malloc(size_of::<Test>(), align_of::<Test>()) as *mut Test;

            alloc.free(p as *mut u8, size_of::<Test>(), align_of::<Test>());

            let p3 = alloc.malloc(size_of::<Test>(), align_of::<Test>()) as *mut Test;

            let curr_heap_end = sbrk(0);
            println!("end {:?}", curr_heap_end);
        }
    }

    // #[test]
    // fn test_malloc() {

    //     unsafe {
    //         let curr_heap_start = sbrk(0);
    //         let mut list = FreeBlockList::new();
    //         let x1 = &mut *malloc::<Test>(&mut list);
    //         x1.field1 = 1;

    //         let x2 = &mut *malloc::<Test>(&mut list);
    //         x2.field1 = 2;

    //         let size = size_of::<Test>();
    //         list.add_free_region(x1 as *mut Test as *mut u8, size);
    //         println!("free");

    //         // let x3 = &mut *malloc::<Test>(&mut list);
    //         // x3.field1 = 3;

    //         let x4 = &mut *malloc::<Thunk>(&mut list);
    //         x4.field2[1] = 2;

    //         let curr_heap_end = sbrk(0);
    //         println!("end {:?}", curr_heap_end);

    //     }
    // }
}
