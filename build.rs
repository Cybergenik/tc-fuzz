fn main() {
    let tc_calc_dir = std::path::Path::new("../tc-calc");
    let ubsan = std::env::var("CARGO_FEATURE_UBSAN").is_ok();

    let mut build = cc::Build::new();

    build
        .file("harness/harness.c")
        .include(tc_calc_dir)
        .compiler("clang")
        .flag("-fsanitize-coverage=edge,trace-pc-guard")
        .flag("-g")
        .flag("-fno-omit-frame-pointer")
        .flag("-O0")
        .warnings(false);

    if ubsan {
        build.flag("-fsanitize=undefined");
    }

    build.compile("tc_calc_harness");

    println!("cargo:rustc-link-lib=m");
    if ubsan {
        println!("cargo:rustc-link-arg=-fsanitize=undefined");
    }
}
