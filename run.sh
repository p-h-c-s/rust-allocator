# enables the sbrk_allocator for specific tests
cargo test --package rmalloc --bin rmalloc --features "use_sbrk_allocator" -- cbindings::sbrk::tests::test_alloc_base --exact --show-output --nocapture


cargo test --package rmalloc --bin rmalloc --features "use_sbrk_allocator" -- cbindings::sbrk::tests::test_malloc_excess --exact --show-output --nocapture


cargo run --features "use_sbrk_allocator" src/main.rs