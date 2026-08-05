mod alloc;
mod report;
mod svg;

#[global_allocator]
static ALLOCATOR: alloc::CountingAllocator = alloc::CountingAllocator;

fn main() {
    // Replaced by the full CLI in Task 23.
    println!("crowd-bench");
}
