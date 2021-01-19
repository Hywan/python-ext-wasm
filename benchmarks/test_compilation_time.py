from wasmer import engine, Store, Module
from wasmer_compiler_cranelift import Compiler as Cranelift
from wasmer_compiler_llvm import Compiler as LLVM
from wasmer_compiler_singlepass import Compiler as Singlepass
import wasmtime

TEST_BYTES = open('benchmarks/qjs.wasm', 'rb').read()

def _test(compiler, benchmark):
    store = Store(engine.JIT(compiler))

    @benchmark
    def bench():
        _ = Module(store, TEST_BYTES)

def test_benchmark_compilation_time_cranelift(benchmark):
    _test(Cranelift, benchmark)

def test_benchmark_compilation_time_llvm(benchmark):
    _test(LLVM, benchmark)

def test_benchmark_compilation_time_singlepass(benchmark):
    _test(Singlepass, benchmark)

def test_benchmark_compilation_time_wasmtime(benchmark):
    config = wasmtime.Config()
    config.strategy = "cranelift"
    engine = wasmtime.Engine(config)
    store = wasmtime.Store(engine)

    @benchmark
    def bench():
        _ = wasmtime.Module(store.engine, TEST_BYTES)
