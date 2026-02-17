use std::env::{Args, args_os};
use std::error::Error;
use std::io;
use std::{fs, result};

use rlox::chunk::Chunk;
use rlox::chunk::*;
use rlox::vm::{InterpretResult, VM};

// TODO: transfer to test module.
fn sample_chunk() {
    // let virtual_machine = VM::init();
    let mut ch: Chunk = Chunk::new();
    // let idx = ch.add_constant(1.2);
    // ch.write_chunk(OpCode::Return, 1);
    ch.write_constant(42.01, 2);
    ch.write_constant(2.0, 2);
    ch.write_chunk(OpCode::Add, 2);
    ch.write_constant(5.6, 2);
    ch.write_chunk(OpCode::Divide, 2);
    ch.write_chunk(OpCode::Negate, 2);
    ch.write_chunk(OpCode::Return, 2);

    // dbg!(&ch);
    ch.disassemble("test bytes");
    // virtual_machine.
}

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

fn interpret(input: &str) -> InterpretResult {
    todo!()
}

fn main() {
    // reading from commandline arguments
    let args: Vec<String> = std::env::args().skip(1).collect::<Vec<String>>();
    let arg_count = args.len();
    if arg_count == 1 {
        repl();
    } else if arg_count == 2 {
        let path = args[1].to_owned();
        todo!()
    } else {
        eprintln!("Usage: rlox path[]");
        std::process::exit(64);
    }
    println!("Hello, world!");
}
