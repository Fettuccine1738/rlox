#[cfg(test)]
pub mod test {
    use rox::{
        compile::compiler::Compiler,
        runtime::vm::{InterpretResult, VM},
    };

    /// this tests in this test suite mostly follow the pattern
    /// ````
    /// let src = "print \"foo\";"
    /// let mut vm = VM::init();
    /// assert_eq!(vm.interpret(src.to_owned()), InterpretResult::Ok);
    /// ````
    macro_rules! assert_interprets_ok {
        ($src:expr) => {{
            let mut vm = VM::init();
            assert_eq!(vm.interpret($src.to_owned()), InterpretResult::Ok);
        }};
    }

    macro_rules! assert_interpreter_expects {
        ($src:expr, $expected:expr) => {{
            let mut vm = VM::init();
            assert_eq!(vm.interpret($src.to_owned()), $expected);
        }};
    }

    macro_rules! assert_file_interprets_ok {
        ($path:expr) => {{
            let base = env!("CARGO_MANIFEST_DIR");
            let full_path = format!("{}/{}", base, $path);
            let src = std::fs::read_to_string(&full_path)
                .unwrap_or_else(|e| panic!("Failed to read test file '{}': {}", full_path, e));
            let mut vm = VM::init();
            assert_eq!(vm.interpret(src), InterpretResult::Ok);
        }};
    }

    macro_rules! assert_file_interpreter_expects {
        ($path:expr, $expected:expr) => {{
            let base = env!("CARGO_MANIFEST_DIR");
            let full_path = format!("{}/{}", base, $path);
            let src = std::fs::read_to_string(&full_path)
                .unwrap_or_else(|e| panic!("Failed to read test file '{}': {}", full_path, e));
            let mut vm = VM::init();
            assert_eq!(vm.interpret(src), $expected);
        }};
    }

    pub mod my_suite {
        use super::*;
        #[test]
        pub(super) fn tests_arithmetic_expr() {
            // TODO: this also tests that a single '5' is stored in the constants pool.
            // debug outputs should show 2 constants saved 5 and 4.
            let source = "print 5 - 4 + 5;";
            let mut vm = VM::init();
            assert_eq!(vm.interpret(source.to_string()), InterpretResult::Ok);
        }

        #[test]
        pub(super) fn tests_assignment() {
            // TODO: this also tests that a single '5' is stored in the constants pool.
            // debug outputs should show 2 constants saved 5 and 4.
            let source = "var simple = 5 - 4 + 5;";
            let mut vm = VM::init();
            assert_eq!(vm.interpret(source.to_string()), InterpretResult::Ok);
        }

        // tests end-end process from, scanning/parsing to
        // compiling and interpreting.
        #[test]
        pub(super) fn tests_compilation() {
            assert_interprets_ok!("!(5 - 4 > 3 * 2 == !nil);") // nil is falsy in lox. !nil = true.
        }

        #[test]
        fn tests_invalid_expr_is_compile_error() {
            assert_interpreter_expects!("1 +;", InterpretResult::CompileError)
        }

        // really annoying to append ';' to simple expressions.
        #[test]
        fn tests_string_concatenation() {
            assert!(Compiler::compile("\"st\" + \"ring\";").is_some());
        }

        #[test]
        fn tests_string_concatenation_exec() {
            assert_interprets_ok!(
                "var b = \"beignets\"; \n\
                         var beverage = \"capuccino\"; \n\
                         var breakfast = \"beignets with \"+ beverage; \n\
                         print breakfast;"
            )
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
            assert_interpreter_expects!(
                "const boo = \"cow\"; \n\
                               boo = \"co\";",
                InterpretResult::CompileError
            );
        }

        /// tests access const global variable compiles successfully.
        #[test]
        fn test_const_variable_access_ok() {
            assert_interprets_ok!(
                "const foo = \"hello\"; \n\
                           var bar = \"\"; \n\
                           bar = foo + \" world.\n\"; \n\
                           print bar; \n\
                           print foo;"
            )
        }

        #[test]
        fn test_block_scope_ok() {
            assert_interprets_ok!(
                "{ var breakfast = \"beignets\"; \n\
                         var beverage = \"capuccino\"; \n\
                         breakfast = \"beignets with \"+ beverage; \n\
                         print breakfast; }"
            )
        }

        /// tests functions (both Lox and Native) that do not take in any argument.
        #[test]
        fn test_noarg_function_call_ok() {
            assert_interprets_ok!(
                "\n\
                    fun areWeHavingItYet() { \n\
                    print \"Yes we are!\";  \n\
                    } \n\
                    var start = time::clock();
                    areWeHavingItYet();
                    print time::clock() - start;
        "
            )
        }

        /// native functions that take arguments.
        #[test]
        fn test_nativefunc_with_args_ok() {
            assert_interprets_ok!(
                "
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
                "
            )
        }

        // TODO: allow string and number concatenation print foo + \" squared = \" + math::pow(foo, 2);
        #[test]
        fn test_while_loop_ok() {
            assert_interprets_ok!(
                "
                var foo = 0;
                const bar = 5;
                while (foo < bar) {
                    print foo + \" squared = \" + math::pow(foo, 2);
                    foo = foo + 1;
                }
                "
            )
        }

        /// tests closure correctly recognizes mutation of captured values.
        #[test]
        fn tests_closures_see_global_mutations() {
            assert_interprets_ok!(
                "var x = \"in global\";

                        fun outer() {
                            fun inner() {
                                print x;
                            }
                            inner();
                        }
                        outer();
                        x= \"global changed.\";
                        outer();
                    "
            )
        }

        /// tests that connection between local value and captured values are not severed.
        /// verifies that a closures see a change to a local value.
        #[test]
        fn tests_closures_see_local_mutations() {
            assert_interprets_ok!(
                "
                        fun outer() {
                            var local = \"buzz\";
                            fun inner() {
                                print local;
                            }
                            local = \"fizz\";
                            inner();
                        }

                        outer();
                    "
            )
        }

        #[test]
        fn tests_nested_functions() {
            assert_interprets_ok!(
                "var x = \"in global\";
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
                    "
            )
        }

        // all objects here is reachable from the roots and should not
        // be collected.
        #[test]
        fn tests_gc_reaches_all() {
            assert_interprets_ok!(
                "
                        fun makeClosure() {
                        var x = \"data\";
                        fun f() {
                            print x;
                        }
                            return f;
                        }
                        var closure = makeClosure();
                        closure();
                    "
            )
        }

        // NOTE: in cases where the class name is already defined
        // No error is thrown. Fix this.
        #[test]
        fn tests_instance_get_field_ok() {
            assert_interprets_ok!(
                "
                        class Pair {}
                        var p = Pair();
                        p.first = 1;
                        p.second = 2;
                        print p.first + p.second;
                     "
            )
        }

        #[test]
        fn test_instance_field_access_ok() {
            assert_interprets_ok!(
                "
        class CoffeeMaker {
          init(coffee) {
            this.coffee = coffee;
          }
          brew() {
            print \"Enjoy your cup of \" + this.coffee;
          }
        }

        class Restaurant {}
        var maker = CoffeeMaker(\"coffee and chicory\");
        maker.brew();

        var rst = Restaurant();
        rst.bev = maker;
        rst.bev.coffee = \"foo\";
        print rst.bev.coffee;
        rst.bev.brew();
    "
            );
        }

        #[test]
        fn test_simple_class_impl() {
            assert_interprets_ok!(
                "
        class CoffeeMaker {
          init(coffee) {
            this.coffee = coffee;
          }
          brew() {
            print \"Enjoy your cup of \" + this.coffee;
            this.coffee = nil;
          }
        }
        var maker = CoffeeMaker(\"coffee and chicory\");
        maker.brew();
    "
            );
        }

        #[test]
        fn tests_instance_set_field_ok() {
            assert_interprets_ok!(
                "
        class Brioche {}
        var b = Brioche();
        b.jam = \"grape\";
        print b.jam;
    "
            );
        }

        #[test]
        fn tests_array_ops_ok() {
            assert_interprets_ok!(
                "
                var foo = \"bar\";
                var arr = [[4.0, \"baz\"],1, 2.0, foo];
                print arr[0][1];

                arr[2] = 3.142;
                print arr[2];
            "
            )
        }

        #[test]
        fn tests_array_access_notok() {
            assert_interpreter_expects!(
                "
                var foo = \"bar\";
                var arr = [[4.0, \"baz\"],1, 2.0, foo];
                print arr[3];
                print arr[0][2];
            ",
                InterpretResult::RuntimeError
            )
        }

        #[test]
        fn tests_super_call_dispatch_ok() {
            assert_interprets_ok!(
                "
            class A {
              method() {
                print \"A method\";
              }
            }

            class B < A {
              method() {
                print \"B method\";
              }

              test() {
                super.method();
              }
            }

            class C < B {}

            C().test();
            B().method();
            "
            );
        }

        #[test]
        fn tests_matrix_mul() {
            assert_interprets_ok!(
                "
                var a = [
                    [1.0, 2.0, 3.0],
                    [3.0, 2.0, 1.0],
                    [1.0, 2.0, 3.0]
                ];

                var b = [
                    [4.0, 5.0, 6.0],
                    [6.0, 5.0, 4.0],
                    [4.0, 6.0, 5.0]
                ];

                var result = [
                    [0.0, 0.0, 0.0],
                    [0.0, 0.0, 0.0],
                    [0.0, 0.0, 0.0]
                ];

                var i = 0;
                while (i < 3) {
                    var j = 0;

                    while (j < 3) {
                        var sum = 0.0;

                        var k = 0;
                        while (k < 3) {
                            sum = sum + a[i][k] * b[k][j];
                            k = k + 1;
                        }

                        result[i][j] = sum;
                        j = j + 1;
                    }

                    i = i + 1;
                }

                print result[0][0];
                print result[1][1];
                print result[2][2];
            "
            )
        }
    }

    pub mod lox_suite {
        use super::*;

        macro_rules! lox_test {
            ($name:ident, $path:expr) => {
                #[test]
                fn $name() {
                    assert_file_interprets_ok!($path);
                }
            };
            // variant for tests expected to fail
            ($name:ident, $path:expr, $expected:expr) => {
                #[test]
                fn $name() {
                    assert_file_interpreter_expects!($path, $expected);
                }
            };
        }

        // variables
        lox_test!(
            collide_with_params,
            "tests/lox_samples/test/variable/collide_with_parameter.lox"
        );
        // --- assignment ---
        lox_test!(
            assignment_global,
            "tests/lox_samples/test/assignment/global.lox"
        );
        lox_test!(
            assignment_local,
            "tests/lox_samples/test/assignment/local.lox"
        );
        lox_test!(
            assignment_undefined,
            "tests/lox_samples/test/assignment/undefined.lox",
            InterpretResult::RuntimeError
        );

        // --- block ---
        lox_test!(block_empty, "tests/lox_samples/test/block/empty.lox");
        lox_test!(block_scope, "tests/lox_samples/test/block/scope.lox");

        // --- call ---
        lox_test!(
            call_bool,
            "tests/lox_samples/test/call/bool.lox",
            InterpretResult::RuntimeError
        );
        lox_test!(
            call_nil,
            "tests/lox_samples/test/call/nil.lox",
            InterpretResult::RuntimeError
        );

        // --- closure ---
        lox_test!(
            closure_open_upvalue,
            "tests/lox_samples/test/closure/open_upvalue.lox"
        );

        // --- if ---
        lox_test!(
            if_dangling_else,
            "tests/lox_samples/test/if/dangling_else.lox"
        );
        lox_test!(if_else, "tests/lox_samples/test/if/else.lox");

        // --- logical_operator ---
        lox_test!(
            logical_and,
            "tests/lox_samples/test/logical_operator/and.lox"
        );
        lox_test!(
            logical_or,
            "tests/lox_samples/test/logical_operator/or.lox"
        );

        // --- while ---
        lox_test!(while_syntax, "tests/lox_samples/test/while/syntax.lox");

        // --- function ---
        lox_test!(
            function_recursion,
            "tests/lox_samples/test/function/recursion.lox"
        );
        lox_test!(
            function_mutual_recursion,
            "tests/lox_samples/test/function/mutual_recursion.lox"
        );
    }
}
