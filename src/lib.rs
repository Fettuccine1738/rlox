pub mod chunk;
pub mod compiler;
pub mod lox_errors;
pub mod value;
pub mod vm;

#[cfg(test)]
pub mod test {
    use crate::vm::{self, InterpretResult};

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
}
