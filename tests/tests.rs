#[cfg(test)]
pub mod test {
    use rlox::{
        compile::compiler::Compiler,
        runtime::vm::{InterpretResult, VM},
    };

    #[test]
    pub(super) fn tests_arithmetic_expr() {
        // TODO: this also tests that a single '5' is stored in the constants pool.
        // debug outputs should show 2 constants saved 5 and 4.
        let source = "print 5 - 4 + 5;";
        let mut vm = VM::new();
        assert_eq!(vm.interpret(source.to_string()), InterpretResult::Ok);
    }

    #[test]
    pub(super) fn tests_assignment() {
        // TODO: this also tests that a single '5' is stored in the constants pool.
        // debug outputs should show 2 constants saved 5 and 4.
        let source = "var simple = 5 - 4 + 5;";
        let mut vm = VM::new();
        assert_eq!(vm.interpret(source.to_string()), InterpretResult::Ok);
    }

    // tests end-end process from, scanning/parsing to
    // compiling and interpreting.
    #[test]
    pub(super) fn tests_compilation() {
        let source = "!(5 - 4 > 3 * 2 == !nil);"; // nil is falsy in lox. !nil = true.
        let mut vm = VM::new();
        let result = vm.interpret(source.to_owned());
        assert_eq!(result, InterpretResult::Ok);
    }

    #[test]
    fn tests_invalid_expr_is_compile_error() {
        let source = "1 +;";
        assert_eq!(
            VM::new().interpret(source.to_owned()),
            InterpretResult::CompileError
        )
    }

    #[test]
    fn tests_invalid_expr_is_runtime_error() {
        let source = "1 + nil;";
        assert_eq!(
            VM::new().interpret(source.to_owned()),
            InterpretResult::RuntimeError
        )
    }

    // really annoying to append ';' to simple expressions.
    #[test]
    fn tests_string_concatenation() {
        assert!(Compiler::compile("\"st\" + \"ring\";").is_some());
    }

    #[test]
    fn tests_string_concatenation_exec() {
        let _src = "var b = \"beignets\"; \n\
                         var beverage = \"capuccino\"; \n\
                         var breakfast = \"beignets with \"+ beverage; \n\
                         print breakfast;";
        assert_eq!(VM::new().interpret(_src.to_owned()), InterpretResult::Ok)
    }

    #[test]
    fn tests_valid_printstmt_successful() {
        let src = "print 1 + 2;";
        assert!(Compiler::compile(src).is_some());
    }

    #[test]
    fn test_unnamed_variable_fails_compile() {
        let src = "foo = \"bar\";"; // variable foo is undeclared.
        assert!(Compiler::compile(src).is_none());
    }

    #[test]
    fn tests_global_declaration_ok() {
        let _src = "var breakfast = \"beignets\"; \n\
                         var beverage = \"capuccino\"; \n\
                         breakfast = \"beignets with \"+ beverage; \n\
                         print breakfast;";
        assert!(Compiler::compile(_src).is_some());
    }

    /// tests that a const declared variable should fail
    /// at compile time when reassigned to.
    #[test]
    fn test_constglobal_declaration_notok() {
        let _src2 = "const boo = \"cow\"; \n\
                               boo = \"co\";";
        assert_eq!(
            VM::new().interpret(_src2.to_owned()),
            InterpretResult::CompileError
        );
    }

    /// tests access const global variable compiles successfully.
    #[test]
    fn test_const_variable_access_ok() {
        let _src2 = "const foo = \"hello\"; \n\
                           var bar = \"\"; \n\
                           bar = foo + \" world.\n\"; \n\
                           print bar; \n\
                           print foo;";
        assert_eq!(VM::new().interpret(_src2.to_owned()), InterpretResult::Ok);
    }

    #[test]
    fn test_block_scope_ok() {
        let _src = "{ var breakfast = \"beignets\"; \n\
                         var beverage = \"capuccino\"; \n\
                         breakfast = \"beignets with \"+ beverage; \n\
                         print breakfast; }";
        assert_eq!(VM::new().interpret(_src.to_owned()), InterpretResult::Ok);
    }

    /// tests functions (both Lox and Native) that do not take in any argument.
    #[test]
    fn test_noarg_function_call_ok() {
        let source = "\n\
                    fun areWeHavingItYet() { \n\
                    print \"Yes we are!\";  \n\
                } \n\
                var start = time::clock();
                areWeHavingItYet();
                print time::clock() - start;
                ";
        assert_eq!(VM::new().interpret(source.to_owned()), InterpretResult::Ok);
    }

    /// native functions that take arguments.
    #[test]
    fn test_nativefunc_with_args_ok() {
        let source = "
                var s1 = \"aoo\";
                var s2 = \"aoo\";
                var comp = strings::str_cmp(s1, s2);
                if (comp == -1.0) {
                    print s1 + \" less \" + s2;
                } else if (comp == 0.0) {
                    print s1 + \" equals \" + s2;
                } else {
                    print s1 + \" greater \" + s2;
                }
                ";
        assert_eq!(VM::new().interpret(source.to_owned()), InterpretResult::Ok);
    }

    // TODO: allow string and number concatenation print foo + \" squared = \" + math::pow(foo, 2);
    #[test]
    fn test_while_loop_ok() {
        let source = "
                var foo = 0;
                const bar = 5;
                while (foo < bar) {
                    print foo + \" squared = \" + math::pow(foo, 2);
                    foo = foo + 1;
                }
                ";
        assert_eq!(VM::new().interpret(source.to_owned()), InterpretResult::Ok);
    }

    /// tests global variable can be reassigned to from closure.
    /// tests closure can capture variables declared in outer and global scopes.
    #[test]
    fn tests_nested_functions() {
        let src = "var x = \"in global\";
                         var y = \"foo\";
                         print y;

                        fun outer() {
                        var x = \"in outer\";
                        fun inner() {
                            y = \"bar\";
                            print x;
                        }
                        inner();
                    }
                    outer();
                    print y;
                    ";
        let mut vm = VM::new();
        assert_eq!(vm.interpret(src.to_owned()), InterpretResult::Ok);
    }
}
