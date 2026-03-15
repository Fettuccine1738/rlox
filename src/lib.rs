pub mod chunk;
pub mod compiler;
pub mod lox_errors;
pub mod value;
pub mod vm;
pub mod data_structures;

#[cfg(test)]
pub mod test {
    use crate::{compiler::Compiler, vm::{self, InterpretResult}, chunk::Chunk};

    // Not a real test, just to walk through the implementation so far.
    #[test]
    pub(super) fn tests_simple_arithmetic_op() {
        let source = "5 - 4";
        let mut ch: Chunk = Chunk::new();
        let success: bool = Compiler::compile(source, &mut ch);
        assert!(success);
    }

    // tests end-end process from, scanning/parsing to
    // compiling and interpreting.
    #[test]
    pub(super) fn tests_compilation() {
        let source = "!(5 - 4 > 3 * 2 == !nil)";
        let mut vm = vm::VM::new();
        let result = vm.compile(source.to_owned());
        assert_eq!(result, InterpretResult::Ok);
    }

    #[test]
    fn tests_compile_error() {
        let source = "1 +";
        let mut vm = vm::VM::new();
        assert_eq!(
            vm.compile(source.to_owned()),
            InterpretResult::CompileError
        )
    }

    #[test]
    fn tests_runtime_error() {
        let source = "1 + nil";
        let mut vm = vm::VM::new();
        assert_eq!(
            vm.compile(source.to_owned()),
            InterpretResult::RuntimeError
        )
    }

    #[test]
    fn tests_string_concatenation() {
        let mut ch: Chunk = Chunk::new();
        assert!(Compiler::compile("\"st\" +   \"ri\" + \"ing\"", &mut ch));
    }
}
