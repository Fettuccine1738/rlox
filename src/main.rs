use rlox::chunk::Chunk;
use rlox::chunk::OpCode::*;

fn main() {
    let mut ch: Chunk = Chunk::new();
    ch.write_chunk(Return);
    dbg!(&ch);

    ch.disassemble("test bytes");
    println!("Hello, world!");
}
