#[cfg(test)]
pub mod test {
    use rlox::{
        core::chunk::Chunk,
        compile::compiler::Compiler,
        core::opcode::OpCode,
        core::value::Value,
        vm::{self, InterpretResult},
    };

    // Not a real test, just to walk through the implementation so far.
    #[test]
    pub(super) fn tests_simple_arithmetic_op() {
        // this also tests that a single '5' is stored in the constants pool.
        // debug outputs should show 2 constants saved 5 and 4.
        let source = "5 - 4 + 5;";
        let mut ch: Chunk = Chunk::new();
        let success: bool = Compiler::compile(source, &mut ch);
        assert!(success);
    }

    // tests end-end process from, scanning/parsing to
    // compiling and interpreting.
    #[test]
    pub(super) fn tests_compilation() {
        let source = "!(5 - 4 > 3 * 2 == !nil);";
        let mut vm = vm::VM::new();
        let result = vm.compile(source.to_owned());
        assert_eq!(result, InterpretResult::Ok);
    }

    #[test]
    fn tests_invalid_expr_is_compile_error() {
        let source = "1 +;";
        let mut vm = vm::VM::new();
        assert_eq!(vm.compile(source.to_owned()), InterpretResult::CompileError)
    }

    // Compilation should be successful.
    #[test]
    fn tests_runtime_error() {
        let source = "1 + nil;";
        let mut vm = vm::VM::new();
        assert_eq!(
            vm.interpret(source.to_owned()),
            InterpretResult::RuntimeError
        )
    }

    // really annoying to append ';' to simple expressions.
    #[test]
    fn tests_string_concatenation() {
        let mut ch: Chunk = Chunk::new();
        assert!(Compiler::compile("\"st\" + \"ring\";", &mut ch));
    }

    #[test]
    fn tests_string_concatenation_exec() {
        let _src = "var b = \"beignets\"; \n\
                         var beverage = \"capuccino\"; \n\
                         var breakfast = \"beignets with \"+ beverage; \n\
                         print breakfast;";
        assert_eq!(
            vm::VM::new().interpret(_src.to_owned()),
            InterpretResult::Ok
        )
    }

    #[test]
    fn tests_valid_printstmt_successful() {
        let mut chunk: Chunk = Chunk::new();
        let src = "print 1 + 2;";
        assert!(Compiler::compile(src, &mut chunk));
    }

    #[test]
    fn tests_global_declaration() {
        let mut chunk: Chunk = Chunk::new();
        let _src = "var breakfast = \"beignets\"; \n\
                         var beverage = \"capuccino\"; \n\
                         breakfast = \"beignets with \"+ beverage; \n\
                         print breakfast;";
        //  var boole = !true; \n\
        let _src2 = "var b = \"cow\";";

        assert!(Compiler::compile(_src, &mut chunk));
    }

    #[test]
    fn test_chunk_orders_byte_ok() {
        // let virtual_machine = VM::init();
        let mut ch: Chunk = Chunk::new();
        // let idx = ch.add_constant(1.2);
        // ch.write_chunk(OpCode::Return, 1);
        ch.write_constant(Value::Number(42.01), 2);
        ch.write_constant(Value::Number(2.0), 2);
        ch.write_chunk(OpCode::Add, 2);
        ch.write_constant(Value::Number(1.0), 2);
        ch.write_chunk(OpCode::Divide, 2);
        ch.write_chunk(OpCode::Negate, 2);
        ch.write_chunk(OpCode::Return, 2);

        // dbg!(&ch);
        Chunk::disassemble(&ch, "test bytes");
        // virtual_machine.
    }
}
