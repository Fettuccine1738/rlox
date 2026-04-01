#[cfg(test)]
pub mod test {
    use rlox::{
        core::chunk::Chunk,
        compile::compiler::Compiler,
        core::opcode::OpCode,
        core::value::Value,
        runtime::vm::{self, InterpretResult},
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

    // this should fail.
    // TODO: force var or const keyword before variable declaration or
    // keep as it is and implicit Var keyword.
    #[test]
    fn test_unnamed_variable_fails_compile() {
        let mut chunk: Chunk = Chunk::new();
        let src = "foo = \"bar\";";
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
        assert!(Compiler::compile(_src, &mut chunk));
    }

    /// tests that a const declared variable should fail 
    /// at compile time when reassigned to. 
    /// TODO: possibly allow this but ignore modification.
    #[test]
    fn test_constglobal_declaration_notok() {
        let _src2 = "const b = \"cow\"; \n\
                               b = \"co\";";
        assert!(!Compiler::compile(_src2, &mut Chunk::new()))
    }


    /// tests access const global variable compiles successfully.
    #[test]
    fn test_constglobal_access_ok() {
        let mut vm = vm::VM::new();
        let _src2 = "const foo = \"hello\"; \n\
                           var bar = \"\"; \n\
                           bar = foo + \" world.\n\"; \n\
                           print bar; \n\
                           print foo;";
        assert_eq!(vm.interpret(_src2.to_owned()), InterpretResult::Ok);
    }

    #[test]
    fn test_local_scopes_ok() {
        let mut chunk: Chunk = Chunk::new();
        let _src = "{ var breakfast = \"beignets\"; \n\
                         var beverage = \"capuccino\"; \n\
                         breakfast = \"beignets with \"+ beverage; \n\
                         print breakfast; }";
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
