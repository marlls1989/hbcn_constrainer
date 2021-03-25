use lalrpop;

fn main() {
    lalrpop::process_root().unwrap();
    if cfg!(feature = "embedded_cbc") {
        println!("cargo:rustc-link-lib=static=stdc++");
    }
}
