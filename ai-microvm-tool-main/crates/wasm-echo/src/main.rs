fn main() {
    let args: Vec<String> = std::env::args().collect();

    println!("hello from wasm-echo");

    if args.len() <= 1 {
        println!("no arguments provided");
        return;
    }

    println!("arguments:");

    for arg in args.iter().skip(1) {
        println!("- {arg}");
    }
}
