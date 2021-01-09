use std::mem::MaybeUninit;
use std::convert::TryInto;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct JitCodeDataPagePair {
    locked: AtomicBool,
    pub contents: *mut libc::c_void,
}

pub const PAGE_SIZE: usize = 4096;

impl<'a> JitCodeDataPagePair {
    pub fn new() -> Self {
        unsafe {
            let mut contents : MaybeUninit<*mut libc::c_void> = MaybeUninit::uninit(); // avoid uninitalized warning
            libc::posix_memalign(contents.as_mut_ptr(), PAGE_SIZE, PAGE_SIZE * 2);
            let contents = contents.assume_init();

            libc::mprotect(contents, PAGE_SIZE, libc::PROT_READ | libc::PROT_WRITE);
            libc::mprotect(
                (contents as *mut u8).offset(PAGE_SIZE as isize) as *mut libc::c_void,
                PAGE_SIZE,
                libc::PROT_READ | libc::PROT_WRITE
            );

            libc::memset(contents, 0xc3, PAGE_SIZE * 2);  // for now, prepopulate with 'RET'

            Self { contents, locked: AtomicBool::new(false) }
        }
    }

    pub unsafe fn lock(&mut self) -> i32 {
        self.locked.store(true, Ordering::SeqCst);

        libc::mprotect(self.contents, PAGE_SIZE, libc::PROT_EXEC | libc::PROT_READ)
    }

    pub unsafe fn unlock(&mut self) -> i32 {
        self.locked.store(false, Ordering::SeqCst);

        libc::mprotect(self.contents, PAGE_SIZE, libc::PROT_WRITE | libc::PROT_READ)
    }

    pub fn get_func_ptr<T>(&self, offset: usize) -> unsafe extern "C" fn() -> T {
        if !self.locked.load(Ordering::SeqCst) {
            panic!("Cannot run unlocked JitCodeDataPagePair");
        }

        unsafe {
            std::mem::transmute((self.contents as *const u8).offset(offset.try_into().unwrap()))
        }
    }

    pub unsafe fn code_as_slice(&mut self) -> &'a mut [u8] {
        if self.locked.load(Ordering::SeqCst) {
            panic!("Cannot edit locked JitCodeDataPagePair");
        }
        std::slice::from_raw_parts_mut(self.contents as _, PAGE_SIZE)
    }

    pub unsafe fn data_as_mut_slice<T: Sized>(&mut self) -> &'a mut [T] {
        std::slice::from_raw_parts_mut(
            (self.contents as *mut u8).offset(PAGE_SIZE as isize) as *mut T,
            PAGE_SIZE / std::mem::size_of::<T>()
        )
    }

    pub unsafe fn data_as_slice<T: Sized>(&self) -> &'a [T] {
        std::slice::from_raw_parts(
            (self.contents as *const u8).offset(PAGE_SIZE as isize) as *const T,
            PAGE_SIZE / std::mem::size_of::<T>()
        )
    }
}
