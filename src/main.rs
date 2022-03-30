use clap::Parser;
use std::{
    error::Error,
    fs::File,
    io::{BufReader, BufWriter, Read},
    path::PathBuf,
};
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
    let mut buf = Vec::new();
    BufReader::new(File::open(args.input)?).read_to_end(&mut buf)?;
    let image_data = qoi_decoder::ImageData::decode(&buf);
    let out_file_buf = BufWriter::new(File::create(args.output)?);
    image_data.write_png_file(out_file_buf)?;
    Ok(())
}
