#![feature(const_mut_refs)] // allows static mut refs
use cbindings::sbrk::{Locked, SbrkAllocator};
use libc::sbrk;

pub mod cbindings;

#[global_allocator]
static GLOBAL: Locked<SbrkAllocator> = Locked::new(SbrkAllocator::new());

struct Test1 {
    a: u8,
}

struct Test2 {
    a: u8,
    b: u8,
}

fn main() {
    unsafe {
        let curr_heap_start = sbrk(0);

        let y = Box::new(Test1 { a: 3 });
        // let z = Box::new(Test2{a: 1, b: 5});
        {
            let b = Box::new(Test1 { a: 1 });
            let h = b.a + 1;
        }
        let x = Box::new(Test1 { a: 1 });

        let curr_heap_end = sbrk(0);
        let diff = curr_heap_end as u8 - curr_heap_start as u8;
        println!("{:?} bytes allocated", diff);
        println!("{:?}", curr_heap_end);
    }
}
