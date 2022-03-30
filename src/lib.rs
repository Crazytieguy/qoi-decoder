#![feature(destructuring_assignment)]
#![feature(array_chunks)]

use std::{
    error::Error,
    io::{BufRead, Write},
};

use derive_new::new;

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
}

const END_MARKER: [u8; 8] = [0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b01];

const QOI_OP_RGB: u8 = 0b11111110;
const QOI_OP_RGBA: u8 = 0b11111111;
const QOI_OP_INDEX: u8 = 0b00;
const QOI_OP_DIFF: u8 = 0b01;
const QOI_OP_LUMA: u8 = 0b10;
const QOI_OP_RUN: u8 = 0b11;

pub struct ImageData {
    header: QOIHeader,
    pixels: Vec<Pixel>,
}

fn get_next_array_chunk<T, const N: usize>(bytes: &[T]) -> Option<(&[T; N], &[T])> {
    Some((bytes.array_chunks().next()?, &bytes[N..]))
}

impl ImageData {
    pub fn decode(mut input_buf: impl BufRead) -> Result<Self, Box<dyn Error>> {
        let mut bytes = Vec::new();
        input_buf.read_to_end(&mut bytes)?;
        let not_enough_bytes = "Not enough bytes to decode";

        let (magic, bytes) = get_next_array_chunk(&bytes).ok_or(not_enough_bytes)?;
        if magic != b"qoif" {
            return Err("Magic bytes are not 'qoif'".into());
        }

        let (width, bytes) = get_next_array_chunk(bytes).ok_or(not_enough_bytes)?;
        let (height, bytes) = get_next_array_chunk(bytes).ok_or(not_enough_bytes)?;
        let (channels, bytes) = bytes.split_first().ok_or(not_enough_bytes)?;
        let (colorspace, bytes) = bytes.split_first().ok_or(not_enough_bytes)?;
        let header = QOIHeader::new(
            u32::from_be_bytes(*width),
            u32::from_be_bytes(*height),
            *channels,
            *colorspace,
        );

        let mut pixels: Vec<Pixel> = Vec::new();
        let mut bytes = bytes;
        let mut color_index_array = [Pixel::new(0, 0, 0, 0); 64];
        let last_pixel = |pixels: &[Pixel]| {
            pixels
                .last()
                .copied()
                .unwrap_or_else(|| Pixel::new(0, 0, 0, 255))
        };

        while pixels.len() < (header.width * header.height) as usize {
            let next_byte;
            (next_byte, bytes) = bytes.split_first().ok_or(not_enough_bytes)?;
            let next_byte = *next_byte;
            let pixel = if next_byte == QOI_OP_RGBA {
                let rgba;
                (rgba, bytes) = get_next_array_chunk(bytes).ok_or(not_enough_bytes)?;
                let [r, g, b, a] = *rgba;
                Pixel::new(r, g, b, a)
            } else if next_byte == QOI_OP_RGB {
                let rgb;
                (rgb, bytes) = get_next_array_chunk(bytes).ok_or(not_enough_bytes)?;
                let [r, g, b] = *rgb;
                Pixel::new(r, g, b, last_pixel(&pixels).a)
            } else if next_byte >> 6 == QOI_OP_INDEX {
                color_index_array[(next_byte & 0b111111) as usize]
            } else if next_byte >> 6 == QOI_OP_DIFF {
                let prev_pixel = last_pixel(&pixels);
                let r_diff = ((next_byte >> 4) & 0b11).wrapping_sub(2);
                let g_diff = ((next_byte >> 2) & 0b11).wrapping_sub(2);
                let b_diff = (next_byte & 0b11).wrapping_sub(2);
                Pixel::new(
                    prev_pixel.r.wrapping_add(r_diff),
                    prev_pixel.g.wrapping_add(g_diff),
                    prev_pixel.b.wrapping_add(b_diff),
                    prev_pixel.a,
                )
            } else if next_byte >> 6 == QOI_OP_LUMA {
                let prev_pixel = last_pixel(&pixels);
                let g_diff = (next_byte & 0b111111).wrapping_sub(32);
                let rb_diff;
                (rb_diff, bytes) = bytes.split_first().ok_or(not_enough_bytes)?;
                let r_diff = g_diff.wrapping_add(*rb_diff >> 4).wrapping_sub(8);
                let b_diff = g_diff.wrapping_add(*rb_diff & 0b1111).wrapping_sub(8);
                Pixel::new(
                    prev_pixel.r.wrapping_add(r_diff),
                    prev_pixel.g.wrapping_add(g_diff),
                    prev_pixel.b.wrapping_add(b_diff),
                    prev_pixel.a,
                )
            } else if next_byte >> 6 == QOI_OP_RUN {
                let prev_pixel = last_pixel(&pixels);
                let run_minus_one = next_byte & 0b111111;
                (0..run_minus_one).for_each(|_| pixels.push(prev_pixel));
                prev_pixel
            } else {
                return Err("Illegal op code".into());
            };
            pixels.push(pixel);
            color_index_array[pixel.hash()] = pixel;
        }

        if bytes != END_MARKER {
            return Err("No valid end marker".into());
        }

        Ok(Self { header, pixels })
    }

    pub fn write_png_file(&self, out_file_buf: impl Write) -> Result<(), Box<dyn Error>> {
        let mut encoder = png::Encoder::new(out_file_buf, self.header.width, self.header.height);
        encoder.set_color(png::ColorType::Rgba);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(
            &self
                .pixels
                .iter()
                .flat_map(|p| [p.r, p.g, p.b, p.a])
                .collect::<Vec<_>>(),
        )?;
        Ok(())
    }
}
