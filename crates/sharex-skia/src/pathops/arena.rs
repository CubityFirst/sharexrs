//! `SkArenaAlloc` for the path ops: objects allocated for one operation,
//! at stable addresses, all freed when the operation ends.

type DropFn = unsafe fn(*mut u8);

pub struct Arena {
    items: Vec<(*mut u8, DropFn)>,
}

impl Default for Arena {
    fn default() -> Self {
        Arena { items: Vec::with_capacity(256) }
    }
}

unsafe fn drop_box<T>(p: *mut u8) {
    drop(unsafe { Box::from_raw(p as *mut T) });
}

impl Arena {
    /// `make<T>()`.
    pub fn make<T>(&mut self, value: T) -> *mut T {
        let p = Box::into_raw(Box::new(value));
        self.items.push((p as *mut u8, drop_box::<T>));
        p
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        for &(p, f) in self.items.iter().rev() {
            unsafe { f(p) };
        }
    }
}
