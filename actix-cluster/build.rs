use std::io::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    // Tell Cargo to rerun this build script if the protocol definitions change
    println!("cargo:rerun-if-changed=proto/");
    
    // Compile the Protocol Buffers definitions
    let proto_files = ["proto/messages.proto"];
    
    tonic_build::configure()
        .build_server(false)  // We don't need gRPC server code
        .out_dir("src/serialization/generated")
        .compile(&proto_files, &["proto/"])?;
    
    // Compile with prost for advanced features
    prost_build::compile_protos(&proto_files, &["proto/"])?;
    
    Ok(())
} 