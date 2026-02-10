use rlox::chunk::Chunk;
use rlox::chunk::*;

fn main() {
    let mut ch: Chunk = Chunk::new();
    // let idx = ch.add_constant(1.2);
    // ch.write_chunk(OpCode::Return, 1);
    ch.write_constant(42.01, 2);
    ch.write_chunk(OpCode::Return, 2);
    // dbg!(&ch);

    ch.disassemble("test bytes");
    println!("Hello, world!");
}
