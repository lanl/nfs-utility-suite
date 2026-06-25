fn main() {
    xdr_codegen::Compiler::new()
        .file("mount_proto.x")
        .file("nfs3_xdr.x")
        .disable_alloc()
        .enable_zcopy()
        .enable_no_alloc()
        .run()
        .expect("That should have worked. :(");
}
