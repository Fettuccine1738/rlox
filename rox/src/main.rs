use std::fs;
use std::io;

use rox::runtime::vm::InterpretResult;

pub fn repl() {
    // let stdin = std::io::stdin();
    let mut input: String = String::new();

    loop {
        println!("> ");
        match io::stdin().read_line(&mut input) {
            Ok(n) => {
                println!("{n} bytes read");
                println!("{input}");
            }
            Err(error) => println!("error: {error}"),
        }
        input.clear();
    }
}

pub fn run_file(path: &str) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Could not open the file: {}", e);
            std::process::exit(74);
        }
    };

    let result: InterpretResult = interpret(&source);
    // let mut virtual_m: VM = VM::new(chunk_);

    match result {
        InterpretResult::CompileError => std::process::exit(65),
        InterpretResult::RuntimeError => std::process::exit(70),
        _ => {}
    }
}

fn interpret(_input: &str) -> InterpretResult {
    todo!()
}

fn main() {
    // // reading from commandline arguments
    // let args: Vec<String> = std::env::args().skip(1).collect::<Vec<String>>();
    // let arg_count = args.len();
    // if arg_count == 1 {
    //     repl();
    // } else if arg_count == 2 {
    //     let _path = args[1].to_owned();
    //     todo!()
    // } else {
    //     eprintln!("Usage: rlox path[]");
    //     std::process::exit(64);
    // }
    use string_interner::StringInterner;

    let mut interner = StringInterner::default();
    let sym0 = interner.get_or_intern("Elephant");
    let sym1 = interner.get_or_intern("Tiger");
    let sym2 = interner.get_or_intern("Horse");
    let sym3 = interner.get_or_intern("Tiger");
    println!("{:?}", sym0);
    assert_ne!(sym0, sym1);
    assert_ne!(sym0, sym2);
    assert_ne!(sym1, sym2);
    assert_eq!(sym1, sym3); // same!
    use string_interner::DefaultStringInterner;
    use string_interner::Symbol;

    let interner = <DefaultStringInterner>::from_iter(["Earth", "Water", "Fire", "Air", "AirFire"]);
    for (sym, str) in &interner {
        println!("{} = {}", sym.to_usize(), str);
    }
}
