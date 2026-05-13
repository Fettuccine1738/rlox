use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rox::runtime::vm;

fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        n => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn bench_fibonacci(c: &mut Criterion) {
    c.bench_function("fib 20", |b| b.iter(|| fibonacci(black_box(20))));
}



fn run_lox(source: &str) {
    let mut vm = vm::VM::new();
    let _result: vm::InterpretResult = vm.interpret(source.to_owned());
    std::hint::black_box(source);
}

const MATRIX_MUL: &str = r#"
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

"#;

fn bench_matrix_mul(c: &mut Criterion) {
    c.bench_function("lox_matrix_mul_3x3", |b| {
        b.iter(|| run_lox(black_box(MATRIX_MUL)))
    });
}

criterion_group!(benches, bench_fibonacci, bench_matrix_mul);
criterion_main!(benches);