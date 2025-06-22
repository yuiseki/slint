fn main() {
    println!("cargo:warning=Simple build - testing Slint UI only");
    
    // Build Slint UI only
    slint_build::compile("scene.slint").unwrap();
    
    println!("cargo:warning=Slint UI compiled successfully");
}