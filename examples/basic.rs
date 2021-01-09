use closure_trampoline::trampoline::*;


use std::io::{stdin, stdout, Read, Write};

fn pause() {
    let mut stdout = stdout();
    stdout.write(b"Press Enter to continue...").unwrap();
    stdout.flush().unwrap();
    stdin().read(&mut [0]).unwrap();
}

fn main() {
    let mut tramp_set = TrampolineSet::new();
    let x = String::from("captured string");
    tramp_set.set_slot_fn(0, Box::new(Box::new(|| { println!("test {}", x) })));

    unsafe {
        let func = tramp_set.get_slot_fn(0);

        dbg!(func);

        //pause();

        func();
    }
}
