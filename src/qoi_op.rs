macro_rules! two_bit_op {
    ( $name:ident, $value:expr ) => {
        #[allow(non_snake_case)]
        pub mod $name {
            pub const START: u8 = $value << 6;
            pub const END: u8 = ($value << 6) | 0b00111111;
        }
    };
}

pub const RGB: u8 = 0b11111110;
pub const RGBA: u8 = 0b11111111;
two_bit_op!(INDEX, 0b00);
two_bit_op!(DIFF, 0b01);
two_bit_op!(LUMA, 0b10);
two_bit_op!(RUN, 0b11);
