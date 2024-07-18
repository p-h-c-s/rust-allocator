/// Equivalent to size + (size % align),
/// but more efficient. Only works on powers of 2, but GlobalAlloc guarantees that.

pub fn to_align(size: usize, align: usize) -> usize {
    (size + align - 1) & !(align - 1)
}