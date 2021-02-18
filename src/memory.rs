use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::ptr::NonNull;

use core::cell::UnsafeCell;

use linked_list_allocator::Heap;

#[cfg(not(test))]
#[global_allocator]
static mut ALLOCATOR: HeapAllocator = HeapAllocator::new();

extern "C" {
    static _heap_bottom: u8;
    static _heap_top: u8;
}

// TODO: highly unsafe, use locks right after MMU is on!
pub struct HeapAllocator(Heap);

impl HeapAllocator {
    pub const fn new() -> Self {
        HeapAllocator(Heap::empty())
    }

    pub fn init(&mut self) {
        unsafe {
            let heap_start = &_heap_bottom as *const _ as usize;
            let heap_end = &_heap_top as *const _ as usize;
    
            let heap_size = heap_end - heap_start;

            self.0.init(heap_start, heap_size);
        }
    }
}

unsafe impl GlobalAlloc for HeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let heap_mut : *mut Heap = &self.0 as *const _ as *mut _;

        (*heap_mut)
            .allocate_first_fit(layout)
            .ok()
            .map_or(0 as *mut u8, |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let heap_mut : *mut Heap = &self.0 as *const _ as *mut _;

        (*heap_mut)
            .deallocate(NonNull::new_unchecked(ptr), layout)
    }
}

pub unsafe fn setup() {
    ALLOCATOR.init();
}