#![forbid(unsafe_code)]

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(copperroute::run(&raw).code());
}
