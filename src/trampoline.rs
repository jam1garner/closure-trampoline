use std::iter;
use crate::jit;

const JMP_SIZE: usize = 8;

const TRAMPOLINE_ENTRYPOINT: [u8; 10] = [
    0x48, 0xff, 0xc0,              // inc    rax
    0xe9, 0x02, 0x00, 0x00, 0x00,  // jmp    0x7
    0x31, 0xc0,                    // xor    eax, eax
];

//const TRAMPOLINE_END: [u8; 19] = [
//    0xe8, 0x00, 0x00, 0x00, 0x00,  // call   0xf
//    0x5b,                          // pop    rbx
//    0x48, 0x83, 0xc3, 0x17,        // add    ebx, 0x17
//    0x48, 0x8d, 0x04, 0xc3,        // lea    rax, [rbx+rax*8]
//    0x48, 0x8b, 0x00,              // mov    rax, QWORD PTR [rax] 
//    0x48, 0xbb,                    // mov    rbx, XXXXXXXXXXXXX
//];

const TRAMPOLINE_END_LEN: usize = 19;

const TRAMPOLINE_END: [u8; TRAMPOLINE_END_LEN] = [
    0xe8, 0x00, 0x00, 0x00, 0x00,  // call   0xf
    0x5b,                          // pop    rbx
    0x48, 0x83, 0xc3, TRAMPOLINE_END_LEN as u8 + 5,        // add    ebx, 0x17
    0x48, 0x8d, 0x04, 0xc3,        // lea    rax, [rbx+rax*8]
    //0x48, 0x89, 0xDF,              // mov    rdi, rbx
    0x48, 0x89, 0xC7,              // mov    rdi, rax
    //0x48, 0x8b, 0x00,              // mov    rdi, QWORD PTR [rax] 
    0x48, 0xbb,                    // mov    rbx, XXXXXXXXXXXXX
];

const SIZE_OF_PTR: usize = 8;

const JMP_RBX: [u8; 2] = [0xff, 0xe3];

const SIZE_OF_TRAMPOLINE_END: usize = TRAMPOLINE_END.len() + SIZE_OF_PTR + JMP_RBX.len();

const NOP: u8 = 0x90;

fn repeat_entrypoint() -> impl Iterator<Item = u8> {
    iter::repeat(TRAMPOLINE_ENTRYPOINT.iter().cloned())
        .take(TRAMPOLINE_CAPACITY)
        .flatten()
        .skip(JMP_SIZE)  // skip jmp on first iteration
}

const FIRST_ENTRYPOINT_SIZE: usize = TRAMPOLINE_ENTRYPOINT.len() - JMP_SIZE;
const ENTRYPOINT_AREA_SIZE: usize = jit::PAGE_SIZE - SIZE_OF_TRAMPOLINE_END;

pub const TRAMPOLINE_CAPACITY: usize = {
    ((ENTRYPOINT_AREA_SIZE - FIRST_ENTRYPOINT_SIZE) / TRAMPOLINE_ENTRYPOINT.len()) + 1
};

const NOP_COUNT: usize = jit::PAGE_SIZE - ((
    (TRAMPOLINE_CAPACITY * TRAMPOLINE_ENTRYPOINT.len()) - JMP_SIZE
) + SIZE_OF_TRAMPOLINE_END);

pub extern "C" fn call(boxed_closure: &&&dyn Fn()) {
    boxed_closure();
}

fn generate_trampolines() -> impl Iterator<Item = u8> {
    let call_bytes = usize::to_ne_bytes(call as *const () as usize).iter().cloned().collect::<Vec<_>>();
    iter::repeat(NOP).take(NOP_COUNT)
        .chain(repeat_entrypoint())
        .chain(TRAMPOLINE_END.iter().cloned())
        .chain(call_bytes.into_iter())
        .chain(JMP_RBX.iter().cloned())
}

pub struct TrampolineSet {
    jit_mem: jit::JitCodeDataPagePair
}

impl<'a> TrampolineSet {
    pub const CAPACITY: usize = TRAMPOLINE_CAPACITY;

    pub fn new() -> Self {
        let mut jit_mem = jit::JitCodeDataPagePair::new();

        unsafe {
            jit_mem.unlock();

            jit_mem.code_as_slice()
                .iter_mut()
                .zip(generate_trampolines())
                .for_each(|(code, trampoline)| {
                    *code = trampoline;
                });
            
            jit_mem.lock();
            
        }

        Self { jit_mem }
    }

    pub fn get_slot_fn(&mut self, index: usize) -> unsafe extern "C" fn() {
        if index >=  TRAMPOLINE_CAPACITY {
            panic!("Index out of bounds of the max number")
        }

        let index = index + 1;

        let offset = ((TRAMPOLINE_CAPACITY - index) * TRAMPOLINE_ENTRYPOINT.len()) + NOP_COUNT;

        self.jit_mem.get_func_ptr(offset)
    }

    pub fn set_slot_fn(&mut self, index: usize, closure: Box<Box<dyn Fn() + 'a>>) {
        unsafe {
            let x = self.jit_mem.data_as_mut_slice();

            x[index] = Box::leak(closure);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_payload() {
        assert_eq!(
            generate_trampolines().collect::<Vec<_>>().len(),
            jit::PAGE_SIZE
        )
    }

    #[test]
    fn test_trampoline_set() {
        let mut tramp_set = TrampolineSet::new();
        let x = 10;
        tramp_set.set_slot_fn(0, Box::new(Box::new(|| { println!("test {}", x) })));

        unsafe {
            let func = tramp_set.get_slot_fn(0);

            dbg!(func);


            func();
        }
    }
}

// e9 02 00 00 00          jmp    0x7
// 31 c0                   xor    eax,eax
// 48 ff c0                inc    rax
//
// e8 00 00 00 00          call   0xf
// 5b                      pop    rbx
// 83 c3 09                add    ebx,0x9
// 48 8d 04 c3             lea    rax,[rbx+rax*8]
// 48 8b 00                mov    rax,QWORD PTR [rax] 
//
