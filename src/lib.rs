#![feature(array_chunks)]

use std::{
    error::Error,
    io::{BufRead, Write},
};

use derive_new::new;

const END_MARKER: [u8; 8] = [0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b01];
const NOT_ENOUGH_BYTES: &str = "Not enough bytes to decode";

#[derive(new)]
struct QOIHeader {
    width: u32,
    height: u32,
    channels: u8,
    colorspace: u8,
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

trait FunParsing {
    fn split_chunk<const N: usize>(&self) -> Result<([u8; N], &Self), Box<dyn Error>>;
    fn split_next(&self) -> Result<(u8, &Self), Box<dyn Error>>;
}

impl FunParsing for [u8] {
    fn split_chunk<const N: usize>(&self) -> Result<([u8; N], &Self), Box<dyn Error>> {
        let &chunk = self.array_chunks().next().ok_or(NOT_ENOUGH_BYTES)?;
        Ok((chunk, &self[N..]))
    }

    fn split_next(&self) -> Result<(u8, &Self), Box<dyn Error>> {
        let (&first, rest) = self.split_first().ok_or(NOT_ENOUGH_BYTES)?;
        Ok((first, rest))
    }
}
pub struct ImageData {
    header: QOIHeader,
    image_data: Vec<u8>,
}

impl ImageData {
    pub fn decode(mut input_buf: impl BufRead) -> Result<Self, Box<dyn Error>> {
        let mut bytes = Vec::new();
        input_buf.read_to_end(&mut bytes)?;

        let (magic, bytes) = bytes.split_chunk()?;
        if &magic != b"qoif" {
            return Err("Magic bytes are not 'qoif'".into());
        }

        let (width, bytes) = bytes.split_chunk()?;
        let (height, bytes) = bytes.split_chunk()?;
        let (channels, bytes) = bytes.split_next()?;
        let (colorspace, bytes) = bytes.split_next()?;
        let header = QOIHeader::new(
            u32::from_be_bytes(width),
            u32::from_be_bytes(height),
            channels,
            colorspace,
        );

        let image_data_len = (header.width * header.height) as usize * 4;
        let mut image_data = Vec::with_capacity(image_data_len);
        let mut bytes = bytes;
        let mut color_index_array = [Pixel::new(0, 0, 0, 0); 64];
        let mut prev_pixel = Pixel::new(0, 0, 0, 255);

        while image_data.len() < image_data_len {
            let (next_byte, remaining) = bytes.split_next()?;
            bytes = remaining;
            let (pixel, remaining) = match next_byte {
                // QOI_OP_RGBA
                0b11111111 => {
                    let ([r, g, b, a], remaining) = remaining.split_chunk()?;
                    (Pixel::new(r, g, b, a), remaining)
                }
                // QOI_OP_RGB
                0b11111110 => {
                    let ([r, g, b], remaining) = remaining.split_chunk()?;
                    (Pixel::new(r, g, b, prev_pixel.a), remaining)
                }
                // QOI_OP_INDEX
                0b00000000..=0b00111111 => {
                    let idx = (next_byte & 0b111111) as usize;
                    (color_index_array[idx], remaining)
                }
                // QOI_OP_DIFF
                0b01000000..=0b01111111 => {
                    let r_diff = ((next_byte >> 4) & 0b11).wrapping_sub(2);
                    let g_diff = ((next_byte >> 2) & 0b11).wrapping_sub(2);
                    let b_diff = (next_byte & 0b11).wrapping_sub(2);
                    (prev_pixel.wrapping_add(r_diff, g_diff, b_diff), remaining)
                }
                // QOI_OP_LUMA
                0b10000000..=0b10111111 => {
                    let g_diff = (next_byte & 0b111111).wrapping_sub(32);
                    let (rb_diff, remaining) = bytes.split_next()?;
                    let r_diff = g_diff.wrapping_add(rb_diff >> 4).wrapping_sub(8);
                    let b_diff = g_diff.wrapping_add(rb_diff & 0b1111).wrapping_sub(8);
                    (prev_pixel.wrapping_add(r_diff, g_diff, b_diff), remaining)
                }
                // QOI_OP_RUN
                0b11000000..=0b11111111 => {
                    let run = (next_byte & 0b111111) + 1;
                    let flat_pixel = prev_pixel.flat();
                    (0..run).for_each(|_| image_data.extend_from_slice(&flat_pixel));
                    continue;
                }
            };
            image_data.extend_from_slice(&pixel.flat());
            color_index_array[pixel.hash()] = pixel;
            prev_pixel = pixel;
            bytes = remaining;
        }

        if bytes != END_MARKER {
            return Err("No valid end marker".into());
        }

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
