#![feature(const_mut_refs)] // allows static mut refs
use allocator::sbrk::{Locked, SbrkAllocator};
use libc::sbrk;

pub mod allocator;

#[cfg(feature = "use_sbrk_allocator")]
#[global_allocator]
static GLOBAL: Locked<SbrkAllocator> = Locked::new(SbrkAllocator::new());

struct Test {
    a: u8,
}

fn main() {
    unsafe {
        let curr_heap_start = sbrk(0);

        let _t1 = Box::new(Test { a: 3 });
        // let z = Box::new(Test2{a: 1, b: 5});
        {
            let b = Box::new(Test { a: 1 });
            let _ = b.a + 1;
        }
        let _t2 = Box::new(Test { a: 1 });

        let curr_heap_end = sbrk(0);
        let diff = curr_heap_end as u8 - curr_heap_start as u8;
        println!("{:?} bytes allocated", diff as isize); // 48 bytes allocated by SbrkAllocator
    }
}
