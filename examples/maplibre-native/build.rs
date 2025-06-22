use std::env;
use std::path::PathBuf;

fn main() {
    // Build Slint UI
    slint_build::compile("scene.slint").unwrap();

    // Check if we should skip MapLibre Native build for faster development
    if env::var("SKIP_MAPLIBRE_BUILD").is_ok() {
        println!("cargo:warning=Skipping MapLibre Native build (SKIP_MAPLIBRE_BUILD set)");
        
        // Still build the CXX bridge but with mock implementation
        cxx_build::bridge("src/lib.rs")
            .file("src/maplibre_bridge.cpp")
            .std("c++17")
            .compile("maplibre_bridge");
        
        return;
    }

    // MapLibre Native repository path
    let maplibre_native_path = "/home/yuiseki/src/github.com/yuiseki/_fork/maplibre-native";
    
    println!("cargo:rerun-if-changed=src/maplibre_bridge.cpp");
    println!("cargo:rerun-if-changed=src/maplibre_bridge.hpp");
    
    // Build MapLibre Native if not already built
    let build_dir = format!("{}/build-linux-opengl", maplibre_native_path);
    let lib_path = format!("{}/libmbgl-core.a", build_dir);
    
    // Check if MapLibre Native repository exists and is properly initialized
    if !std::path::Path::new(maplibre_native_path).exists() {
        panic!("MapLibre Native repository not found at: {}\n\
               Please clone MapLibre Native:\n\
               git clone https://github.com/maplibre/maplibre-native.git {}", 
               maplibre_native_path, maplibre_native_path);
    }
    
    // Check if submodules are initialized
    let vendor_path = format!("{}/vendor/maplibre-native-base/deps/cheap-ruler-cpp/include", maplibre_native_path);
    if !std::path::Path::new(&vendor_path).exists() {
        println!("cargo:warning=MapLibre Native submodules not initialized. Initializing...");
        
        let output = std::process::Command::new("git")
            .args(&["submodule", "update", "--init", "--recursive"])
            .current_dir(maplibre_native_path)
            .output()
            .expect("Failed to initialize MapLibre Native submodules");
        
        if !output.status.success() {
            panic!("Failed to initialize MapLibre Native submodules: {}\n\
                   Please manually run: cd {} && git submodule update --init --recursive", 
                   String::from_utf8_lossy(&output.stderr), maplibre_native_path);
        }
    }
    
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