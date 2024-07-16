use std::{mem::{self, size_of}, ptr::{self, NonNull}};
use libc::sbrk;

// 1 + 8 + 8 = 17
// Allign is alligned by the largest. Largest is 8, so we must allocate
// 24 bytes (3 * 8)
#[repr(C)]
pub struct MemBlockMeta{
    free: bool,
    size: usize,
    next: Option<NonNull<MemBlockMeta>>
}

pub struct MemBlockList {
    tail: Option<NonNull<MemBlockMeta>>
}

/// Allocates total_size = data_size + block_size bytes
/// Returns a pointer to the start of 0..total_size
/// 
impl MemBlockMeta {
    unsafe fn new(data_size: usize) -> *mut Self {
        let block_size = size_of::<Self>() as usize;
        let total_size = block_size + data_size;

        let allignment = total_size + (total_size % mem::align_of::<Self>());
        println!("allignment {:?}", allignment);
        let ptr = sbrk(allignment as i32);
        if ptr == (-1_isize) as *mut libc::c_void {
            panic!("sbrk failed");
        };
        let mem_block = ptr as *mut MemBlockMeta;
        (*mem_block).size = data_size; // error: allignment -> mem_block is not alligned properly
        (*mem_block).free = false;
        (*mem_block).next = None;
        mem_block
    }
    
}

// maybe use https://stackoverflow.com/questions/56613171/does-casting-pointers-in-rust-have-the-same-behavior-as-reinterpret-cast-in-c
#[derive(Debug)]
pub struct Test {
    x: u32,
    arr: [u32; 10]
}

pub unsafe fn get_memory(inc: usize) -> *mut i32 {
    // rethink these casts, might be dangerous
    let i = inc as i32;
    // interprets the returned pointer as the memory increment
    let new_break = sbrk(i) as *mut i32;
    new_break
    // the space between new_break and new_break+inc is our memory space
}

pub unsafe fn allocate<T>(list: &mut MemBlockList) -> *mut MemBlockMeta {
    let p = MemBlockMeta::new(size_of::<T>());
    if list.tail.is_none() {
        list.tail = NonNull::new(p);
        return p
    }
    (*p).next = list.tail;
    list.tail = NonNull::new(p);
    p
}

pub unsafe fn find_free_block<T>(list: &mut MemBlockList) -> *mut MemBlockMeta {
    if list.tail.is_none() {
        let first_p = allocate::<T>(list);
        list.tail = NonNull::new(first_p);
        return first_p
    }
    let first = list.tail.unwrap();
    let mut p = first.as_ptr();
    while !(*p).free {
        match (*p).next {
            Some(ptr) => p = ptr.as_ptr(),
            None => {
                return allocate::<T>(list);
            }
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use std::{mem::size_of, ptr::NonNull, mem::align_of};

    use super::{allocate, find_free_block, get_memory, MemBlockList, MemBlockMeta, Test};



    // #[test]
    // fn test_allocate() {
    //     unsafe {
    //         let p = allocate(size_of::<Test>()) as *mut Test;
    //         let obj = &mut *p;
    //         obj.x = 4;
    //         obj.y = 3;
    //         println!("{:?}", obj);
    //     }
    // }

    #[test]
    fn test_allign() {
        let size = size_of::<Test>();
        unsafe {
            let b = size_of::<MemBlockMeta>();
            println!("{:?}", b);
            let b = align_of::<MemBlockMeta>();
            println!("allign: {:?}", b);
        }
    }

    #[test]
    fn test_new() {
        let size = size_of::<Test>();
        unsafe {
            let mut list = MemBlockList{
                tail: None
            };
            let p = &mut *(allocate::<Test>(&mut list) as *mut Test);
            p.x = 4;
            p.arr = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
            println!("{:?}", size_of::<Test>());
            println!("{:?}", p);
        }
    }

    #[test]
    fn test_find_free_block() {
        let size = size_of::<Test>();
        unsafe {
            let mut list = MemBlockList{
                tail: None
            };
            let p = find_free_block::<Test>(&mut list);
            (&mut *(p as *mut Test)).x = 4;
            println!("pref: {:?}", p);
            println!("tail: {:?}", list.tail);
            // let p_ref = &*(p as *mut Test);

            let p2 = find_free_block::<Test>(&mut list);
            println!("pref2: {:?}", p2);
            (*p).free = true;

            let p3 = find_free_block::<Test>(&mut list);
            println!("val3: {:?}", (&mut *(p3 as *mut Test)));
            println!("pref3: {:?}", p3);
        }
    }

}