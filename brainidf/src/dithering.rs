//! Gamma-correction lookup table (γ = 2.2) with optional 8-phase ordered dithering.
//! Adapted from the original C/C++ implementation.

#[derive(Copy, Clone)]
struct GammaData {
    /// Pre-computed γ-corrected value (0-255)
    value: u8,
    /// 8-bit pattern used for ordered-dither (“1” means bump the output by +1
    /// on the corresponding sub-frame).
    dither: u8,
}

/// 256-entry table; index with the *uncorrected* 8-bit value.
/// Generated with `GammaGenerator.kt` (same as in the original project).
const TABLE_2_2: [GammaData; 256] = [
    GammaData {
        value: 0,
        dither: 0b00000000,
    },
    GammaData {
        value: 0,
        dither: 0b00000000,
    },
    GammaData {
        value: 0,
        dither: 0b00000000,
    },
    GammaData {
        value: 0,
        dither: 0b00000000,
    },
    GammaData {
        value: 0,
        dither: 0b00000000,
    },
    GammaData {
        value: 0,
        dither: 0b00000000,
    },
    GammaData {
        value: 0,
        dither: 0b10000000,
    },
    GammaData {
        value: 0,
        dither: 0b10000000,
    },
    GammaData {
        value: 0,
        dither: 0b10000000,
    },
    GammaData {
        value: 0,
        dither: 0b10000000,
    },
    GammaData {
        value: 0,
        dither: 0b10001000,
    },
    GammaData {
        value: 0,
        dither: 0b10001000,
    },
    GammaData {
        value: 0,
        dither: 0b10001000,
    },
    GammaData {
        value: 0,
        dither: 0b10101000,
    },
    GammaData {
        value: 0,
        dither: 0b10101000,
    },
    GammaData {
        value: 0,
        dither: 0b10101010,
    },
    GammaData {
        value: 0,
        dither: 0b11101010,
    },
    GammaData {
        value: 0,
        dither: 0b11101010,
    },
    GammaData {
        value: 0,
        dither: 0b11101110,
    },
    GammaData {
        value: 0,
        dither: 0b11111110,
    },
    GammaData {
        value: 0,
        dither: 0b11111111,
    },
    GammaData {
        value: 1,
        dither: 0b00000000,
    },
    GammaData {
        value: 1,
        dither: 0b10000000,
    },
    GammaData {
        value: 1,
        dither: 0b10001000,
    },
    GammaData {
        value: 1,
        dither: 0b10101000,
    },
    GammaData {
        value: 1,
        dither: 0b10101010,
    },
    GammaData {
        value: 1,
        dither: 0b11101010,
    },
    GammaData {
        value: 1,
        dither: 0b11111110,
    },
    GammaData {
        value: 1,
        dither: 0b11111111,
    },
    GammaData {
        value: 2,
        dither: 0b10000000,
    },
    GammaData {
        value: 2,
        dither: 0b10001000,
    },
    GammaData {
        value: 2,
        dither: 0b10101010,
    },
    GammaData {
        value: 2,
        dither: 0b11101010,
    },
    GammaData {
        value: 2,
        dither: 0b11111110,
    },
    GammaData {
        value: 3,
        dither: 0b00000000,
    },
    GammaData {
        value: 3,
        dither: 0b10001000,
    },
    GammaData {
        value: 3,
        dither: 0b10101000,
    },
    GammaData {
        value: 3,
        dither: 0b11101010,
    },
    GammaData {
        value: 3,
        dither: 0b11111110,
    },
    GammaData {
        value: 4,
        dither: 0b10000000,
    },
    GammaData {
        value: 4,
        dither: 0b10101000,
    },
    GammaData {
        value: 4,
        dither: 0b11101010,
    },
    GammaData {
        value: 4,
        dither: 0b11111110,
    },
    GammaData {
        value: 5,
        dither: 0b10000000,
    },
    GammaData {
        value: 5,
        dither: 0b10101000,
    },
    GammaData {
        value: 5,
        dither: 0b11101010,
    },
    GammaData {
        value: 5,
        dither: 0b11111110,
    },
    GammaData {
        value: 6,
        dither: 0b10000000,
    },
    GammaData {
        value: 6,
        dither: 0b10101010,
    },
    GammaData {
        value: 6,
        dither: 0b11101110,
    },
    GammaData {
        value: 7,
        dither: 0b10000000,
    },
    GammaData {
        value: 7,
        dither: 0b10101000,
    },
    GammaData {
        value: 7,
        dither: 0b11101110,
    },
    GammaData {
        value: 8,
        dither: 0b00000000,
    },
    GammaData {
        value: 8,
        dither: 0b10101000,
    },
    GammaData {
        value: 8,
        dither: 0b11101110,
    },
    GammaData {
        value: 9,
        dither: 0b10000000,
    },
    GammaData {
        value: 9,
        dither: 0b10101010,
    },
    GammaData {
        value: 9,
        dither: 0b11101110,
    },
    GammaData {
        value: 10,
        dither: 0b10000000,
    },
    GammaData {
        value: 10,
        dither: 0b11101010,
    },
    GammaData {
        value: 10,
        dither: 0b11111111,
    },
    GammaData {
        value: 11,
        dither: 0b10101000,
    },
    GammaData {
        value: 11,
        dither: 0b11101110,
    },
    GammaData {
        value: 12,
        dither: 0b10000000,
    },
    GammaData {
        value: 12,
        dither: 0b11101010,
    },
    GammaData {
        value: 13,
        dither: 0b00000000,
    },
    GammaData {
        value: 13,
        dither: 0b10101010,
    },
    GammaData {
        value: 13,
        dither: 0b11111110,
    },
    GammaData {
        value: 14,
        dither: 0b10101000,
    },
    GammaData {
        value: 14,
        dither: 0b11111110,
    },
    GammaData {
        value: 15,
        dither: 0b10001000,
    },
    GammaData {
        value: 15,
        dither: 0b11101110,
    },
    GammaData {
        value: 16,
        dither: 0b10001000,
    },
    GammaData {
        value: 16,
        dither: 0b11101110,
    },
    GammaData {
        value: 17,
        dither: 0b10001000,
    },
    GammaData {
        value: 17,
        dither: 0b11101110,
    },
    GammaData {
        value: 18,
        dither: 0b10001000,
    },
    GammaData {
        value: 18,
        dither: 0b11111110,
    },
    GammaData {
        value: 19,
        dither: 0b10101000,
    },
    GammaData {
        value: 19,
        dither: 0b11111110,
    },
    GammaData {
        value: 20,
        dither: 0b10101010,
    },
    GammaData {
        value: 21,
        dither: 0b00000000,
    },
    GammaData {
        value: 21,
        dither: 0b11101010,
    },
    GammaData {
        value: 22,
        dither: 0b10000000,
    },
    GammaData {
        value: 22,
        dither: 0b11101110,
    },
    GammaData {
        value: 23,
        dither: 0b10101000,
    },
    GammaData {
        value: 23,
        dither: 0b11111111,
    },
    GammaData {
        value: 24,
        dither: 0b10101010,
    },
    GammaData {
        value: 25,
        dither: 0b10000000,
    },
    GammaData {
        value: 25,
        dither: 0b11101110,
    },
    GammaData {
        value: 26,
        dither: 0b10101000,
    },
    GammaData {
        value: 27,
        dither: 0b10000000,
    },
    GammaData {
        value: 27,
        dither: 0b11101110,
    },
    GammaData {
        value: 28,
        dither: 0b10101000,
    },
    GammaData {
        value: 29,
        dither: 0b00000000,
    },
    GammaData {
        value: 29,
        dither: 0b11101110,
    },
    GammaData {
        value: 30,
        dither: 0b10101000,
    },
    GammaData {
        value: 31,
        dither: 0b10000000,
    },
    GammaData {
        value: 31,
        dither: 0b11101110,
    },
    GammaData {
        value: 32,
        dither: 0b10101010,
    },
    GammaData {
        value: 33,
        dither: 0b10001000,
    },
    GammaData {
        value: 33,
        dither: 0b11111111,
    },
    GammaData {
        value: 34,
        dither: 0b11101110,
    },
    GammaData {
        value: 35,
        dither: 0b10101010,
    },
    GammaData {
        value: 36,
        dither: 0b10001000,
    },
    GammaData {
        value: 36,
        dither: 0b11111111,
    },
    GammaData {
        value: 37,
        dither: 0b11101110,
    },
    GammaData {
        value: 38,
        dither: 0b10101010,
    },
    GammaData {
        value: 39,
        dither: 0b10001000,
    },
    GammaData {
        value: 40,
        dither: 0b10000000,
    },
    GammaData {
        value: 40,
        dither: 0b11111110,
    },
    GammaData {
        value: 41,
        dither: 0b11101110,
    },
    GammaData {
        value: 42,
        dither: 0b10101010,
    },
    GammaData {
        value: 43,
        dither: 0b10101000,
    },
    GammaData {
        value: 44,
        dither: 0b10001000,
    },
    GammaData {
        value: 45,
        dither: 0b10000000,
    },
    GammaData {
        value: 45,
        dither: 0b11111110,
    },
    GammaData {
        value: 46,
        dither: 0b11101110,
    },
    GammaData {
        value: 47,
        dither: 0b11101010,
    },
    GammaData {
        value: 48,
        dither: 0b11101010,
    },
    GammaData {
        value: 49,
        dither: 0b10101010,
    },
    GammaData {
        value: 50,
        dither: 0b10101000,
    },
    GammaData {
        value: 51,
        dither: 0b10001000,
    },
    GammaData {
        value: 52,
        dither: 0b10001000,
    },
    GammaData {
        value: 53,
        dither: 0b10000000,
    },
    GammaData {
        value: 54,
        dither: 0b10000000,
    },
    GammaData {
        value: 55,
        dither: 0b00000000,
    },
    GammaData {
        value: 55,
        dither: 0b11111111,
    },
    GammaData {
        value: 56,
        dither: 0b11111111,
    },
    GammaData {
        value: 57,
        dither: 0b11111110,
    },
    GammaData {
        value: 58,
        dither: 0b11111110,
    },
    GammaData {
        value: 59,
        dither: 0b11111110,
    },
    GammaData {
        value: 60,
        dither: 0b11111110,
    },
    GammaData {
        value: 61,
        dither: 0b11111110,
    },
    GammaData {
        value: 62,
        dither: 0b11111110,
    },
    GammaData {
        value: 63,
        dither: 0b11111111,
    },
    GammaData {
        value: 65,
        dither: 0b00000000,
    },
    GammaData {
        value: 66,
        dither: 0b00000000,
    },
    GammaData {
        value: 67,
        dither: 0b10000000,
    },
    GammaData {
        value: 68,
        dither: 0b10000000,
    },
    GammaData {
        value: 69,
        dither: 0b10001000,
    },
    GammaData {
        value: 70,
        dither: 0b10101000,
    },
    GammaData {
        value: 71,
        dither: 0b10101000,
    },
    GammaData {
        value: 72,
        dither: 0b10101010,
    },
    GammaData {
        value: 73,
        dither: 0b11101010,
    },
    GammaData {
        value: 74,
        dither: 0b11101110,
    },
    GammaData {
        value: 75,
        dither: 0b11111110,
    },
    GammaData {
        value: 77,
        dither: 0b00000000,
    },
    GammaData {
        value: 78,
        dither: 0b10001000,
    },
    GammaData {
        value: 79,
        dither: 0b10101000,
    },
    GammaData {
        value: 80,
        dither: 0b10101010,
    },
    GammaData {
        value: 81,
        dither: 0b11101110,
    },
    GammaData {
        value: 82,
        dither: 0b11111110,
    },
    GammaData {
        value: 84,
        dither: 0b10000000,
    },
    GammaData {
        value: 85,
        dither: 0b10001000,
    },
    GammaData {
        value: 86,
        dither: 0b10101010,
    },
    GammaData {
        value: 87,
        dither: 0b11101110,
    },
    GammaData {
        value: 88,
        dither: 0b11111111,
    },
    GammaData {
        value: 90,
        dither: 0b10001000,
    },
    GammaData {
        value: 91,
        dither: 0b10101010,
    },
    GammaData {
        value: 92,
        dither: 0b11101110,
    },
    GammaData {
        value: 93,
        dither: 0b11111111,
    },
    GammaData {
        value: 95,
        dither: 0b10001000,
    },
    GammaData {
        value: 96,
        dither: 0b10101010,
    },
    GammaData {
        value: 97,
        dither: 0b11111110,
    },
    GammaData {
        value: 99,
        dither: 0b10000000,
    },
    GammaData {
        value: 100,
        dither: 0b10101010,
    },
    GammaData {
        value: 101,
        dither: 0b11111110,
    },
    GammaData {
        value: 103,
        dither: 0b10000000,
    },
    GammaData {
        value: 104,
        dither: 0b10101010,
    },
    GammaData {
        value: 105,
        dither: 0b11111110,
    },
    GammaData {
        value: 107,
        dither: 0b10001000,
    },
    GammaData {
        value: 108,
        dither: 0b11101010,
    },
    GammaData {
        value: 109,
        dither: 0b11111111,
    },
    GammaData {
        value: 111,
        dither: 0b10101000,
    },
    GammaData {
        value: 112,
        dither: 0b11101110,
    },
    GammaData {
        value: 114,
        dither: 0b10001000,
    },
    GammaData {
        value: 115,
        dither: 0b11101010,
    },
    GammaData {
        value: 117,
        dither: 0b10000000,
    },
    GammaData {
        value: 118,
        dither: 0b10101010,
    },
    GammaData {
        value: 119,
        dither: 0b11111111,
    },
    GammaData {
        value: 121,
        dither: 0b10101000,
    },
    GammaData {
        value: 122,
        dither: 0b11111110,
    },
    GammaData {
        value: 124,
        dither: 0b10101000,
    },
    GammaData {
        value: 125,
        dither: 0b11111110,
    },
    GammaData {
        value: 127,
        dither: 0b10101000,
    },
    GammaData {
        value: 128,
        dither: 0b11111110,
    },
    GammaData {
        value: 130,
        dither: 0b10101000,
    },
    GammaData {
        value: 131,
        dither: 0b11111110,
    },
    GammaData {
        value: 133,
        dither: 0b10101010,
    },
    GammaData {
        value: 135,
        dither: 0b00000000,
    },
    GammaData {
        value: 136,
        dither: 0b11101010,
    },
    GammaData {
        value: 138,
        dither: 0b10000000,
    },
    GammaData {
        value: 139,
        dither: 0b11101110,
    },
    GammaData {
        value: 141,
        dither: 0b10101000,
    },
    GammaData {
        value: 142,
        dither: 0b11111110,
    },
    GammaData {
        value: 144,
        dither: 0b10101010,
    },
    GammaData {
        value: 146,
        dither: 0b10000000,
    },
    GammaData {
        value: 147,
        dither: 0b11101110,
    },
    GammaData {
        value: 149,
        dither: 0b10101000,
    },
    GammaData {
        value: 151,
        dither: 0b10000000,
    },
    GammaData {
        value: 152,
        dither: 0b11101110,
    },
    GammaData {
        value: 154,
        dither: 0b10101000,
    },
    GammaData {
        value: 156,
        dither: 0b10000000,
    },
    GammaData {
        value: 157,
        dither: 0b11101110,
    },
    GammaData {
        value: 159,
        dither: 0b10101010,
    },
    GammaData {
        value: 161,
        dither: 0b10000000,
    },
    GammaData {
        value: 162,
        dither: 0b11111110,
    },
    GammaData {
        value: 164,
        dither: 0b11101010,
    },
    GammaData {
        value: 166,
        dither: 0b10101000,
    },
    GammaData {
        value: 168,
        dither: 0b10000000,
    },
    GammaData {
        value: 169,
        dither: 0b11111110,
    },
    GammaData {
        value: 171,
        dither: 0b11101010,
    },
    GammaData {
        value: 173,
        dither: 0b10101000,
    },
    GammaData {
        value: 175,
        dither: 0b10001000,
    },
    GammaData {
        value: 176,
        dither: 0b11111111,
    },
    GammaData {
        value: 178,
        dither: 0b11101110,
    },
    GammaData {
        value: 180,
        dither: 0b11101010,
    },
    GammaData {
        value: 182,
        dither: 0b10101010,
    },
    GammaData {
        value: 184,
        dither: 0b10001000,
    },
    GammaData {
        value: 186,
        dither: 0b10000000,
    },
    GammaData {
        value: 187,
        dither: 0b11111111,
    },
    GammaData {
        value: 189,
        dither: 0b11111110,
    },
    GammaData {
        value: 191,
        dither: 0b11101110,
    },
    GammaData {
        value: 193,
        dither: 0b11101010,
    },
    GammaData {
        value: 195,
        dither: 0b10101010,
    },
    GammaData {
        value: 197,
        dither: 0b10101000,
    },
    GammaData {
        value: 199,
        dither: 0b10101000,
    },
    GammaData {
        value: 201,
        dither: 0b10001000,
    },
    GammaData {
        value: 203,
        dither: 0b10001000,
    },
    GammaData {
        value: 205,
        dither: 0b10000000,
    },
    GammaData {
        value: 207,
        dither: 0b10000000,
    },
    GammaData {
        value: 209,
        dither: 0b10000000,
    },
    GammaData {
        value: 211,
        dither: 0b10000000,
    },
    GammaData {
        value: 213,
        dither: 0b00000000,
    },
    GammaData {
        value: 215,
        dither: 0b00000000,
    },
    GammaData {
        value: 217,
        dither: 0b10000000,
    },
    GammaData {
        value: 219,
        dither: 0b10000000,
    },
    GammaData {
        value: 221,
        dither: 0b10000000,
    },
    GammaData {
        value: 223,
        dither: 0b10000000,
    },
    GammaData {
        value: 225,
        dither: 0b10001000,
    },
    GammaData {
        value: 227,
        dither: 0b10001000,
    },
    GammaData {
        value: 229,
        dither: 0b10101000,
    },
    GammaData {
        value: 231,
        dither: 0b10101000,
    },
    GammaData {
        value: 233,
        dither: 0b10101010,
    },
    GammaData {
        value: 235,
        dither: 0b11101010,
    },
    GammaData {
        value: 237,
        dither: 0b11101110,
    },
    GammaData {
        value: 239,
        dither: 0b11111110,
    },
    GammaData {
        value: 241,
        dither: 0b11111111,
    },
    GammaData {
        value: 244,
        dither: 0b10000000,
    },
    GammaData {
        value: 246,
        dither: 0b10001000,
    },
    GammaData {
        value: 248,
        dither: 0b10101010,
    },
    GammaData {
        value: 250,
        dither: 0b11101010,
    },
    GammaData {
        value: 252,
        dither: 0b11101110,
    },
    GammaData {
        value: 255,
        dither: 0b00000000,
    },
];

/// Bare look-up (no dithering).
///
/// Equivalent to `Gamma::Correct22NoDither` in the original code.
#[inline(always)]
pub fn correct_22_no_dither(value: u8) -> u8 {
    TABLE_2_2[value as usize].value
}

/// Look-up **with** 8-phase ordered dithering (frame-balanced).
///
/// * `frame_number` is a monotonically increasing counter (e.g. video-frame ID).  
/// * `pixel_index` can be anything that decorrelates neighbouring pixels
///   (x + y, LED index, etc.).
#[inline(always)]
pub fn correct_22(value: u8, frame_number: u32, pixel_index: u32) -> u8 {
    let entry = TABLE_2_2[value as usize];
    let dither_offset = ((frame_number + pixel_index) & 0x07) as u8; // % 8
    if (entry.dither & (1 << dither_offset)) != 0 {
        entry.value.saturating_add(1)
    } else {
        entry.value
    }
}
