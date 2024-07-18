#![feature(const_mut_refs)] // allows static mut refs
use cbindings::sbrk::{Allocator, Locked};
use libc::sbrk;

pub mod cbindings;

#[global_allocator]
static GLOBAL: Locked<Allocator> = Locked::new(Allocator::new());

fn main() {
    // let l = cbindings::sbrk::FreeBlockList::new();
    unsafe {
        let curr_heap_start = sbrk(0);

        {
            let item = 4;
            let x = Box::new(item);
            let mut v = vec![1, 2, 3];
            v[0] = 1;
        }

        let curr_heap_end = sbrk(0);
        println!("{:?}", curr_heap_end);
    }
}
