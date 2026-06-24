use rzd_core::Mulberry32;

fn main() {
    let mut rng = Mulberry32::new(42);
    println!("RZD-DES-ENGINE CLI initialized.");
    println!("First random number: {}", rng.next_u32());
}
