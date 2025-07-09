fn main() {
    xdr_codegen::Compiler::new()
        .file("protocol_definitions/rpcbind.x")
        .file("protocol_definitions/rpc_prot.x")
        .run()
        .expect("That should have worked. :(");
}
