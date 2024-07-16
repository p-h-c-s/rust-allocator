ref: https://gee.cs.oswego.edu/dl/html/malloc.html
https://gist.github.com/kvark/f067ba974446f7c5ce5bd544fe370186
https://manishearth.github.io/blog/2021/03/15/arenas-in-rust/
https://web.mit.edu/rust-lang_v1.25/arch/amd64_ubuntu1404/share/doc/rust/html/book/first-edition/raw-pointers.html

https://danluu.com/malloc-tutorial/

For people less familiar with lifetimes: the lifetimes in the syntaxes &'a Foo and Foo<'b> mean different things. 'a in &'a Foo is the lifetime of Foo, or, at least the lifetime of this reference to Foo. 'b in Foo<'b> is a lifetime parameter of Foo, and typically means something like “the lifetime of data Foo is allowed to reference”.

A good memory allocator needs to balance a number of goals:
Maximizing Compatibility
An allocator should be plug-compatible with others; in particular it should obey ANSI/POSIX conventions.
Maximizing Portability
Reliance on as few system-dependent features (such as system calls) as possible, while still providing optional support for other useful features found only on some systems; conformance to all known system constraints on alignment and addressing rules.
Minimizing Space
The allocator should not waste space: It should obtain as little memory from the system as possible, and should maintain memory in ways that minimize fragmentation -- ``holes''in contiguous chunks of memory that are not used by the program.
Minimizing Time
The malloc(), free() and realloc routines should be as fast as possible in the average case.
Maximizing Tunability
Optional features and behavior should be controllable by users either statically (via #define and the like) or dynamically (via control commands such as mallopt).
Maximizing Locality
Allocating chunks of memory that are typically used together near each other. This helps minimize page and cache misses during program execution.
Maximizing Error Detection
It does not seem possible for a general-purpose allocator to also serve as general-purpose memory error testing tool such as Purify. However, allocators should provide some means for detecting corruption due to overwriting memory, multiple frees, and so on.
Minimizing Anomalies
An allocator configured using default settings should perform well across a wide range of real loads that depend heavily on dynamic allocation -- windowing toolkits, GUI applications, compilers, interpretors, development tools, network (packet)-intensive programs, graphics-intensive packages, web browsers, string-processing applications, and so on.


Build first allocator with sbkr, and then upgrade to mmap
https://stackoverflow.com/questions/2076532/how-does-sbrk-work-in-c
https://stackoverflow.com/questions/55768549/in-malloc-why-use-brk-at-all-why-not-just-use-mmap



Memory has to be contigous, so we don't free memory to the OS, we soft-free it in the eyes of the program. A linked list allows us to manage that