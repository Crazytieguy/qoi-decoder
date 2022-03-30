use std::{
    fs::File,
    io::{self, BufReader, Read},
};

use itertools::Itertools;

fn main() -> io::Result<()> {
    let file = File::open("qoi_test_images/dice.qoi")?;
    println!("{}", 0xF8F7F6);
    BufReader::new(file)
        .bytes()
        .map(Result::unwrap)
        .next_tuple()
        .map(|(a, b, c, d)| u32::from_be_bytes([a, b, c, d]))
        .unwrap();
    Ok(())
}
