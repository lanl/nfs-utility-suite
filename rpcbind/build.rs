fn main() {
    xdr_codegen::Compiler::new()
        .file("rpcbind.x")
        .run()
        .expect("That should have worked. :(");
}

