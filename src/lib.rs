#![feature(array_chunks)]

use std::{
    error::Error,
    io::{Read, Write},
};

use derive_new::new;
use nom::{
    bits::{bits, complete::take},
    bytes::complete::tag,
    combinator::map,
    number::complete::{be_u32, be_u8},
    sequence::{preceded, tuple},
    IResult,
};
use qoi_op_codes::*;
mod qoi_op_codes;

const END_MARKER: [u8; 8] = [0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b01];

#[allow(dead_code)]
#[derive(new)]
struct QOIHeader {
    width: u32,
    height: u32,
    channels: u8,
    colorspace: u8,
}

impl QOIHeader {
    fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, (width, height, channels, colorspace)) =
            preceded(tag(b"qoif"), tuple((be_u32, be_u32, be_u8, be_u8)))(input)?;
        Ok((input, Self::new(width, height, channels, colorspace)))
    }
}

#[derive(new, Clone, Copy)]
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Pixel {
    fn hash(&self) -> usize {
        (self.r as usize * 3 + self.g as usize * 5 + self.b as usize * 7 + self.a as usize * 11)
            % 64
    }

    fn wrapping_add(&self, r: u8, g: u8, b: u8) -> Self {
        Self::new(
            self.r.wrapping_add(r),
            self.g.wrapping_add(g),
            self.b.wrapping_add(b),
            self.a,
        )
    }

    fn flat(&self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

pub struct ImageData {
    header: QOIHeader,
    image_data: Vec<u8>,
}

impl ImageData {
    pub fn decode(mut input_buf: impl Read) -> Result<Self, Box<dyn Error>> {
        let mut bytes = Vec::new();
        input_buf.read_to_end(&mut bytes)?;
        let (bytes, header) = QOIHeader::parse(&bytes).map_err(|e| e.to_owned())?;
        let image_data_len = (header.width * header.height) as usize * 4;
        let (_, image_data) = parse_image_data(bytes, image_data_len).map_err(|e| e.to_owned())?;
        Ok(Self { header, image_data })
    }

    pub fn write_png_file(&self, out_file_buf: impl Write) -> Result<(), Box<dyn Error>> {
        let mut encoder = png::Encoder::new(out_file_buf, self.header.width, self.header.height);
        encoder.set_color(png::ColorType::Rgba);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&self.image_data)?;
        Ok(())
    }
}

macro_rules! skip_two_bits {
    ($parser:expr) => {
        bits::<_, _, nom::error::Error<(&[u8], usize)>, _, _>(preceded::<_, u8, _, _, _, _>(
            take(2_usize),
            $parser,
        ))
    };
}

fn parse_image_data(mut bytes: &[u8], image_data_len: usize) -> IResult<&[u8], Vec<u8>> {
    let mut image_data = Vec::with_capacity(image_data_len);
    let mut color_index_array = [Pixel::new(0, 0, 0, 0); 64];
    let mut prev_pixel = Pixel::new(0, 0, 0, 255);
    let n_bit_diff = |n: usize| map(take(n), move |diff: u8| diff.wrapping_sub(1 << (n - 1)));
    while image_data.len() < image_data_len {
        let (rest, next_byte) = be_u8(bytes)?;
        let (rest, pixel) = match next_byte {
            RGB => {
                let parse_chunk = tuple((be_u8, be_u8, be_u8));
                let to_pixel = |(r, g, b)| Pixel::new(r, g, b, prev_pixel.a);
                map(parse_chunk, to_pixel)(rest)?
            }
            RGBA => {
                let parse_chunk = tuple((be_u8, be_u8, be_u8, be_u8));
                let to_pixel = |(r, g, b, a)| Pixel::new(r, g, b, a);
                map(parse_chunk, to_pixel)(rest)?
            }
            INDEX::START..=INDEX::END => {
                let parse_chunk = take(6_usize);
                let to_pixel = |idx: usize| color_index_array[idx];
                skip_two_bits!(map(parse_chunk, to_pixel))(bytes)?
            }
            DIFF::START..=DIFF::END => {
                let parse_chunk = tuple((n_bit_diff(2), n_bit_diff(2), n_bit_diff(2)));
                let to_pixel = |(dr, dg, db)| prev_pixel.wrapping_add(dr, dg, db);
                skip_two_bits!(map(parse_chunk, to_pixel))(bytes)?
            }
            LUMA::START..=LUMA::END => {
                let parse_chunk = tuple((n_bit_diff(6), n_bit_diff(4), n_bit_diff(4)));
                let to_pixel = |(dg, drdg, dbdg): (u8, u8, u8)| {
                    let dr = dg.wrapping_add(drdg);
                    let db = dg.wrapping_add(dbdg);
                    prev_pixel.wrapping_add(dr, dg, db)
                };
                skip_two_bits!(map(parse_chunk, to_pixel))(bytes)?
            }
            RUN::START..=RUN::END => {
                let (rest, run) = skip_two_bits!(map(take(6_usize), |v: usize| v + 1))(bytes)?;
                let flat_pixel = prev_pixel.flat();
                (0..run).for_each(|_| image_data.extend_from_slice(&flat_pixel));
                bytes = rest;
                continue;
            }
        };
        bytes = rest;
        image_data.extend_from_slice(&pixel.flat());
        color_index_array[pixel.hash()] = pixel;
        prev_pixel = pixel;
    }
    let (bytes, _) = tag(END_MARKER)(bytes)?;
    Ok((bytes, image_data))
}
