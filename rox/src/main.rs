use std::env;
use std::fs;
use std::io;

use rox::runtime::vm;
use rox::runtime::vm::InterpretResult;
use rox::runtime::vm::VM;

pub const COMPILE_ERR_CODE: i32 = 65;
pub const RUNTIME_ERR_CODE: i32 = 70;

pub fn repl(vm: &mut VM) {
    loop {
        let mut source: String = String::new();
        print!("> ");
        match io::stdin().read_line(&mut source) {
            Ok(_) => (),
            Err(error) => eprintln!("error: {error}"),
        }
        match vm.interpret(source) {
            InterpretResult::CompileError => std::process::exit(COMPILE_ERR_CODE),
            InterpretResult::RuntimeError => std::process::exit(RUNTIME_ERR_CODE),
            InterpretResult::Ok => (),
        }
    }
}

pub fn run_file(path: &str, vm: &mut VM) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Could not open the file: {}", e);
            std::process::exit(74);
        }
    };

    let result: InterpretResult = vm.interpret(source);

    match result {
        InterpretResult::CompileError => std::process::exit(COMPILE_ERR_CODE),
        InterpretResult::RuntimeError => std::process::exit(RUNTIME_ERR_CODE),
        _ => {}
    }
}

fn main() {
    let mut vm: VM = vm::VM::new();
    let args: Vec<String> = env::args().skip(1).collect::<Vec<String>>();

    if args.len() == 0 {
        println!("Starting REPL");
        repl(&mut vm);
    } else if args.len() == 1 {
        println!("Running file");
        run_file(&args[0], &mut vm);
    } else {
        eprintln!("ROX does not expect more than one argument.");
    }
}
