# rust-allocator
Simple rust memory-allocator implementations. 
Implemented in order to learn memory allocation internals and unsafe rust.

### Using allocator as global allocator
The `use_sbrk_allocator` feature defines wether rust should use the custom allocator as the global allocator (for all objects)

### Rust-analyzer and features
To use rust-analyzer nicely, define
```json
"rust-analyzer.cargo.features": "all"
```
