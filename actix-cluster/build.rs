use std::io::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    // Tell Cargo to rerun this build script if the protocol definitions change
    println!("cargo:rerun-if-changed=src/proto/");

    // Compile the Protocol Buffers definitions
    let proto_files = ["src/proto/messages.proto"];

    // Compile with prost for advanced features
    prost_build::compile_protos(&proto_files, &["src/"])?;

    Ok(())
}