use std::env;
use std::fs;
use std::io;
use std::io::BufRead;

use rox::runtime::vm;
use rox::runtime::vm::InterpretResult;
use rox::runtime::vm::VM;

pub const COMPILE_ERR_CODE: i32 = 65;
pub const RUNTIME_ERR_CODE: i32 = 70;
pub const FILEIO_ERR_CODE: i32 = 74;

pub fn repl(vm: &mut VM) {
    loop {
        println!(">> ");
        let mut source = String::new();
        let stdin = io::stdin();

        loop {
            let mut line = String::new();
            match stdin.lock().read_line(&mut line) {
                Ok(0) => break, 
                Ok(_) => {
                    if line.trim().is_empty() {
                        break;
                    }
                    source.push_str(&line);
                }
                Err(error) => eprintln!("error: {error}"),
            }
        }

        if source.is_empty() {
            continue;
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
            std::process::exit(FILEIO_ERR_CODE);
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
    let mut vm: VM = vm::VM::init();
    let args: Vec<String> = env::args().skip(1).collect::<Vec<String>>();

    if args.len() == 0 {
        repl(&mut vm);
    } else if args.len() == 1 {
        run_file(&args[0], &mut vm);
    } else {
        eprintln!("Rox does not expect more than one argument.");
    }
}
