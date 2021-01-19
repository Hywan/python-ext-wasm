from wasmer import engine, Store, Module, Instance, wasi
from wasmer_compiler_cranelift import Compiler as Cranelift
from wasmer_compiler_llvm import Compiler as LLVM
from wasmer_compiler_singlepass import Compiler as Singlepass
import wasmtime

TEST_BYTES = open('benchmarks/qjs.wasm', 'rb').read()

def _test(compiler, benchmark):
    store = Store(engine.JIT(compiler))
    module = Module(store, TEST_BYTES)

    wasi_version = wasi.get_version(module, strict=True)
    wasi_env = wasi.StateBuilder('qjs').argument('--eval').argument('console.log("hello")').finalize()

    import_object = wasi_env.generate_import_object(store, wasi_version)

    instance = Instance(module, import_object)
    main = instance.exports._start

    @benchmark
    def bench():
        _ = main()


def test_benchmark_execution_time_cranelift(benchmark):
    _test(Cranelift, benchmark)

def test_benchmark_execution_time_llvm(benchmark):
    _test(LLVM, benchmark)

def test_benchmark_execution_time_singlepass(benchmark):
    _test(Singlepass, benchmark)

def test_benchmark_execution_time_wasmtime(benchmark):
    config = wasmtime.Config()
    config.strategy = "cranelift"
    engine = wasmtime.Engine(config)
    store = wasmtime.Store(engine)

    @benchmark
    def bench():
        _ = wasmtime.Module(store.engine, TEST_BYTES)
