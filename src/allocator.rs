use core::alloc::GlobalAlloc;

use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};

pub const HEAP_START: usize = 0x20000000 + 192 * 1024;
pub const HEAP_SIZE: usize = 64 * 1024;

#[global_allocator]
static ALLOCATOR: BumpAllocatorRef = BumpAllocatorRef::new();

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            heap_start: HEAP_START,
            heap_end: HEAP_SIZE + HEAP_START,
            next: 0,
            allocations: 0,
        }
    }
}

pub struct BumpAllocatorRef(Mutex<CriticalSectionRawMutex, core::cell::RefCell<BumpAllocator>>);

impl BumpAllocatorRef {
    pub const fn new() -> Self {
        Self(Mutex::new(core::cell::RefCell::new(BumpAllocator::new())))
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    let remainder = addr % align;
    if remainder == 0 {
        addr // addr already aligned
    } else {
        addr - remainder + align
    }
}

unsafe impl GlobalAlloc for BumpAllocatorRef {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.0.lock(|alloc| {
            let alloc_start = align_up(alloc.borrow().next, layout.align());
            let alloc_end = match alloc_start.checked_add(layout.size()) {
                Some(end) => end,
                None => return core::ptr::null_mut(),
            };

            if alloc_end > alloc.borrow().heap_end {
                return core::ptr::null_mut(); // out of memory
            }

            alloc.borrow_mut().next += layout.size();
            alloc.borrow_mut().allocations += 1;
            alloc_start as *mut u8
        })
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        self.0.lock(|alloc| {
            alloc.borrow_mut().allocations -= 1;
            if alloc.borrow().allocations == 0 {
                alloc.borrow_mut().next = alloc.borrow().heap_start;
            }
        })
    }
}
