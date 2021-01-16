use closure_trampoline::trampoline::*;

fn main() {
    let mut tramp_set = TrampolineSet::new();

    let x = String::from("captured string");
    tramp_set.set_slot_fn(0, Box::new(Box::new(|| { println!("test {}", x) })));

    let func: unsafe extern "C" fn() = tramp_set.get_slot_fn(0);

    unsafe {
        func();
    }
}
