use rlox::chunk::Chunk;
use rlox::chunk::*;
use rlox::vm::VM;

fn main() {
    // let virtual_machine = VM::init();
    let mut ch: Chunk = Chunk::new();
    // let idx = ch.add_constant(1.2);
    // ch.write_chunk(OpCode::Return, 1);
    ch.write_constant(42.01, 2);
    ch.write_chunk(OpCode::Negate, 2);
    ch.write_chunk(OpCode::Return, 2);
    // dbg!(&ch);
    ch.disassemble("test bytes");
    // virtual_machine.
    println!("Hello, world!");
}
