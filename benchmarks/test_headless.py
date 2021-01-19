from wasmer import engine, Store, Module, Instance, wasi
from wasmer_compiler_cranelift import Compiler as Cranelift
from wasmer_compiler_llvm import Compiler as LLVM
from wasmer_compiler_singlepass import Compiler as Singlepass
import wasmtime

TEST_BYTES = open('benchmarks/qjs.wasm', 'rb').read()

def _test(compiler, benchmark):
    store = Store(engine.JIT(Cranelift))
    module = Module(store, TEST_BYTES)
    serialized = module.serialize()

    @benchmark
    def bench():
        deserialized = Module.deserialize(store, serialized)
        wasi_version = wasi.get_version(module, strict=True)
        wasi_env = wasi.StateBuilder('qjs').argument('--eval').argument('console.log("hello")').finalize()

        import_object = wasi_env.generate_import_object(store, wasi_version)

        _ = Instance(module, import_object)


def test_benchmark_headless_cranelift(benchmark):
    _test(Cranelift, benchmark)

def test_benchmark_headless_llvm(benchmark):
    _test(LLVM, benchmark)

def test_benchmark_headless_singlepass(benchmark):
    _test(Singlepass, benchmark)

def test_benchmark_headless_wasmtime(benchmark):
    config = wasmtime.Config()
    config.strategy = 'auto'
    engine = wasmtime.Engine(config)
    store = wasmtime.Store(engine)

    wasi_config = wasmtime.WasiConfig()
    wasi_config.argv = ['--help']
    wasi = wasmtime.WasiInstance(store, 'wasi_unstable', wasi_config)

    module = wasmtime.Module(wasi.store.engine, TEST_BYTES)
    serialized = module.serialize()

    @benchmark
    def bench():
        deserialized = wasmtime.Module.deserialize(wasi.store.engine, serialized)

        imports = []

        for import_ in module.imports:
            imports.append(wasi.bind(import_))

        _ = wasmtime.Instance(store, module, imports)
