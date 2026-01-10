/// Trait for efficient ZigZag encoding and decoding of signed integers.
pub trait ZigZag: Sized + Copy {
    type Unsigned;

    /// Encodes a signed integer into an unsigned integer.
    /// -1 -> 1, 0 -> 0, 1 -> 2, etc.
    fn zigzag_encode(self) -> Self::Unsigned;

    /// Decodes an unsigned integer back into a signed integer (inverse of encode).
    fn zigzag_decode(encoded: Self::Unsigned) -> Self;
}

// Macro for implementing
macro_rules! impl_zigzag {
    ($signed:ty, $unsigned:ty) => {
        impl ZigZag for $signed {
            type Unsigned = $unsigned;

            #[inline(always)]
            fn zigzag_encode(self) -> Self::Unsigned {
                const BITS: u32 = <$signed>::BITS;
                // (n << 1) ^ (n >> 31)
                // Uses arithmetic shift (>>), which preserves the sign bit.
                ((self << 1) ^ (self >> (BITS - 1))) as $unsigned
            }

            #[inline(always)]
            fn zigzag_decode(encoded: Self::Unsigned) -> Self {
                // (n >> 1) ^ -(n & 1)
                // -(n & 1) is either 0 (if even) or -1 (all bits 1, if odd)
                (encoded >> 1) as $signed ^ -((encoded & 1) as $signed)
            }
        }
    };
}

// Implementations for common types
impl_zigzag!(i8, u8);
impl_zigzag!(i16, u16);
impl_zigzag!(i32, u32);
impl_zigzag!(i64, u64);
impl_zigzag!(isize, usize);

#[cfg(test)]
mod tests {
    use super::ZigZag;
    #[test]
    fn test_zigzag() {
        let enc = (-1i8).zigzag_encode();
        assert_eq!(enc, 1u8);
        let dec = i8::zigzag_decode(enc);
        assert_eq!(dec, -1i8);

        let enc = (0i16).zigzag_encode();
        assert_eq!(enc, 0u16);
        let dec = i16::zigzag_decode(enc);
        assert_eq!(dec, 0i16);

        let enc = (1i32).zigzag_encode();
        assert_eq!(enc, 2u32);
        let dec = i32::zigzag_decode(enc);
        assert_eq!(dec, 1i32);

        let enc = (-12345i64).zigzag_encode();
        assert_eq!(enc, 24689u64);
        let dec = i64::zigzag_decode(enc);
        assert_eq!(dec, -12345i64);
    }
}

/// Trait for writing VarInts
/// Allows generic VarInt writing for unsigned integer types.
pub trait VarIntWritable: Copy + PartialOrd {
    /// Returns whether the value is zero.
    fn is_zero(self) -> bool;

    /// Returns the lower 7 bits as a byte.
    fn low_7_bits(self) -> u8;

    /// Shifts the value right by 7 bits (in-place).
    fn shr_7(&mut self);
}

macro_rules! impl_varint_writable {
    ($($t:ty),*) => {
        $(
            impl VarIntWritable for $t {
                #[inline(always)]
                fn is_zero(self) -> bool {
                    self == 0
                }

                #[inline(always)]
                fn low_7_bits(self) -> u8 {
                    (self & 0x7F) as u8
                }

                #[inline(always)]
                fn shr_7(&mut self) {
                    *self >>= 7;
                }
            }
        )*
    };
}

impl_varint_writable!(u8, u16, u32, u64, usize, u128);
