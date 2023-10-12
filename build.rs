fn main() {
    build_data::set_GIT_BRANCH();
    build_data::set_GIT_COMMIT();
    build_data::set_GIT_DIRTY();
    build_data::set_SOURCE_TIMESTAMP();
    build_data::no_debug_rebuilds();

    protobuf_codegen_pure::Codegen::new()
    .out_dir("protos/src")
    .inputs(["protos/src/common.proto", "protos/src/ledger.proto", "protos/src/consensus.proto"])
    .include("protos/src")
    .run()
    .expect("protobuf codegen failed!!!");
}

