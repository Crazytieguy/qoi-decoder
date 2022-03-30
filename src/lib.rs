use derive_new::new;
use itertools::Itertools;

#[derive(new)]
struct QOIHeader {
    magic: [u8; 4],
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

struct ImageData {
    header: QOIHeader,
    pixels: Vec<Pixel>,
}

const END_MARKER: [u8; 8] = [0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b01];

const QOI_OP_RGB: u8 = 0b11111110;
const QOI_OP_RGBA: u8 = 0b11111111;
const QOI_OP_INDEX: u8 = 0b00;
const QOI_OP_DIFF: u8 = 0b01;
const QOI_OP_LUMA: u8 = 0b10;
const QOI_OP_RUN: u8 = 0b11;

fn decode(bytes: &mut impl Iterator<Item = u8>) -> ImageData {
    let header = QOIHeader::new(
        bytes
            .next_tuple()
            .map(|(a, b, c, d)| [a, b, c, d])
            .expect("header too short"),
        bytes
            .next_tuple()
            .map(|(a, b, c, d)| u32::from_be_bytes([a, b, c, d]))
            .expect("header too short"),
        bytes
            .next_tuple()
            .map(|(a, b, c, d)| u32::from_be_bytes([a, b, c, d]))
            .expect("header too short"),
        bytes.next().expect("header too short"),
        bytes.next().expect("header too short"),
    );
    assert_eq!(&header.magic, b"qoif");
    let mut pixels: Vec<Pixel> = Vec::new();
    while pixels.len() < (header.width * header.height) as usize {
        let next_byte = bytes.next().expect("image data too short");
        if next_byte == QOI_OP_RGB {
            pixels.push(Pixel::new(
                bytes.next().expect("image data too short"),
                bytes.next().expect("image data too short"),
                bytes.next().expect("image data too short"),
                pixels.last().expect("QOI_OP_RGB on first pixel").a,
            ))
        } else if next_byte == QOI_OP_RGBA {
            pixels.push(Pixel::new(
                bytes.next().expect("image data too short"),
                bytes.next().expect("image data too short"),
                bytes.next().expect("image data too short"),
                bytes.next().expect("image data too short"),
            ))
        } else if next_byte >> 6 == QOI_OP_INDEX {
            
        }
    }

    ImageData { header, pixels }
}
