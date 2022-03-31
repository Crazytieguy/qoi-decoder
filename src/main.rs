use clap::Parser;
use std::{error::Error, fs::File, path::PathBuf};
/// A Quite Ok Image format decoder.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// file to decode
    input: PathBuf,

    /// output path
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();
    let input_reader = File::open(args.input)?;
    let image_data = qoi_decoder::ImageData::decode(input_reader)?;
    let out_writer = File::create(args.output)?;
    image_data.write_png_file(out_writer)?;
    Ok(())
}
