fn main() {
    xdr_codegen::Compiler::new()
        .file("mount_proto.x")
        .file("nfs3_xdr.x")
        .run()
        .expect("That should have worked. :(");
}
