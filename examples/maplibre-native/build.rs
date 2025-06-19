use std::env;
use std::path::PathBuf;

fn main() {
    // Build Slint UI
    slint_build::compile("scene.slint").unwrap();

    // MapLibre Native repository path
    let maplibre_native_path = "/home/yuiseki/src/github.com/yuiseki/_fork/maplibre-native";
    
    println!("cargo:rerun-if-changed=src/maplibre_bridge.cpp");
    println!("cargo:rerun-if-changed=src/maplibre_bridge.hpp");
    
    // Build MapLibre Native if not already built
    let build_dir = format!("{}/build-linux-opengl", maplibre_native_path);
    let lib_path = format!("{}/libmbgl-core.a", build_dir);
    
    // Check if library exists, if not, build it
    if !std::path::Path::new(&lib_path).exists() {
        println!("cargo:warning=Building MapLibre Native...");
        
        // Configure CMake
        let output = std::process::Command::new("cmake")
            .args(&["--preset", "linux-opengl-core"])
            .current_dir(maplibre_native_path)
            .output()
            .expect("Failed to configure MapLibre Native");
        
        if !output.status.success() {
            panic!("Failed to configure MapLibre Native: {}", String::from_utf8_lossy(&output.stderr));
        }
        
        // Build the core library
        let output = std::process::Command::new("cmake")
            .args(&["--build", "build-linux-opengl", "--target", "mbgl-core"])
            .current_dir(maplibre_native_path)
            .output()
            .expect("Failed to build MapLibre Native");
        
        if !output.status.success() {
            panic!("Failed to build MapLibre Native: {}", String::from_utf8_lossy(&output.stderr));
        }
    }

    // Create C++ bridge
    let bridge_cpp = "src/maplibre_bridge.cpp";
    
    // Build the C++ bridge
    cxx_build::bridge("src/lib.rs")
        .file(bridge_cpp)
        .include(format!("{}/include", maplibre_native_path))
        .include(format!("{}/vendor/mapbox-base/include", maplibre_native_path))
        .include(format!("{}/vendor/vector-tile/include", maplibre_native_path))
        .include(format!("{}/vendor/protozero/include", maplibre_native_path))
        .include(format!("{}/vendor/boost", maplibre_native_path))
        .std("c++17")
        .compile("maplibre_bridge");

    // Link MapLibre Native static library
    println!("cargo:rustc-link-search=native={}", build_dir);
    println!("cargo:rustc-link-lib=static=mbgl-core");
    
    // Link system dependencies
    println!("cargo:rustc-link-lib=pthread");
    println!("cargo:rustc-link-lib=dl");
    println!("cargo:rustc-link-lib=GL");
    println!("cargo:rustc-link-lib=EGL");
    
    // For debugging
    println!("cargo:warning=MapLibre Native integration configured");
}