#![feature(split_array)]
#![feature(destructuring_assignment)]

use std::{error::Error, io::Write};

use derive_new::new;

#[derive(new)]
pub struct QOIHeader {
    pub width: u32,
    pub height: u32,
    pub channels: u8,
    pub colorspace: u8,
}

#[derive(new, Clone, Copy)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
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
    pub header: QOIHeader,
    pub pixels: Vec<Pixel>,
}

impl ImageData {
    pub fn decode(bytes: &[u8]) -> Self {
        let (magic, bytes) = bytes.split_array_ref();
        assert_eq!(magic, b"qoif");
        let (width, bytes) = bytes.split_array_ref();
        let (height, bytes) = bytes.split_array_ref();
        let (channels, bytes) = bytes.split_first().unwrap();
        let (colorspace, bytes) = bytes.split_first().unwrap();

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
            (next_byte, bytes) = bytes.split_first().unwrap();
            let next_byte = *next_byte;
            let pixel = if next_byte == QOI_OP_RGBA {
                let rgba;
                (rgba, bytes) = bytes.split_array_ref();
                let [r, g, b, a] = *rgba;
                Pixel { r, g, b, a }
            } else if next_byte == QOI_OP_RGB {
                let rgb;
                (rgb, bytes) = bytes.split_array_ref();
                let [r, g, b] = *rgb;
                Pixel {
                    r,
                    g,
                    b,
                    a: last_pixel(&pixels).a,
                }
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
                (rb_diff, bytes) = bytes.split_first().unwrap();
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
                panic!()
            };
            pixels.push(pixel);
            color_index_array[pixel.hash()] = pixel;
        }

        assert_eq!(bytes, END_MARKER);

        Self { header, pixels }
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
