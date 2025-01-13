//! Direct Rust port the "bcdec.h - v0.98"
//!
//! https://github.com/iOrange/bcdec/blob/main/bcdec.h
//!
//! # CREDITS
//!
//! Aras Pranckevicius (@aras-p)
//! - BC1/BC3 decoders optimizations (up to 3x the speed)
//! - BC6H/BC7 bits pulling routines optimizations
//! - optimized BC6H by moving unquantize out of the loop
//! - Split BC6H decompression function into 'half' and
//!   'float' variants
//!
//! Michael Schmidt (@RunDevelopment)
//! - Found better "magic" coefficients for integer interpolation
//!   of reference colors in BC1 color block, that match with
//!   the floating point interpolation. This also made it faster
//!   than integer division by 3!
//!
//! # License
//!
//! This is free and unencumbered software released into the public domain.
//!
//! Anyone is free to copy, modify, publish, use, compile, sell, or
//! distribute this software, either in source code form or as a compiled
//! binary, for any purpose, commercial or non-commercial, and by any
//! means.
//!
//! In jurisdictions that recognize copyright laws, the author or authors
//! of this software dedicate any and all copyright interest in the
//! software to the public domain. We make this dedication for the benefit
//! of the public at large and to the detriment of our heirs and
//! successors. We intend this dedication to be an overt act of
//! relinquishment in perpetuity of all present and future rights to this
//! software under copyright law.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//! MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
//! IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
//! OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
//! ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
//! OTHER DEALINGS IN THE SOFTWARE.
//!
//! For more information, please refer to <https://unlicense.org>

#[inline(always)]
pub(crate) fn decode_block_bc1(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    decode_color_block::<false>(compressed_block, decompressed_block, destination_pitch);
}

#[inline(always)]
pub(crate) fn decode_block_bc2(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    decode_color_block::<true>(
        &compressed_block[8..],
        decompressed_block,
        destination_pitch,
    );
    decode_sharp_alpha_block(compressed_block, decompressed_block, destination_pitch);
}

#[inline(always)]
pub(crate) fn decode_block_bc3(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    decode_color_block::<true>(
        &compressed_block[8..],
        decompressed_block,
        destination_pitch,
    );
    decode_smooth_alpha_block::<4>(
        compressed_block,
        &mut decompressed_block[3..],
        destination_pitch,
    );
}

#[inline(always)]
pub(crate) fn decode_block_bc4(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    decode_smooth_alpha_block::<1>(compressed_block, decompressed_block, destination_pitch);
}

#[inline(always)]
pub(crate) fn decode_block_bc5(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    decode_smooth_alpha_block::<2>(compressed_block, decompressed_block, destination_pitch);
    decode_smooth_alpha_block::<2>(
        &compressed_block[8..],
        &mut decompressed_block[1..],
        destination_pitch,
    );
}

/// Decompresses a BC1/DXT1 color block
#[inline(always)]
fn decode_color_block<const OPAQUE_MODE: bool>(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    let mut ref_colors = [0u32; 4];
    let c0 = u16::from_le_bytes([compressed_block[0], compressed_block[1]]);
    let c1 = u16::from_le_bytes([compressed_block[2], compressed_block[3]]);

    // Unpack 565 ref colors
    let r0 = (c0 >> 11) & 0x1F;
    let g0 = (c0 >> 5) & 0x3F;
    let b0 = c0 & 0x1F;

    let r1 = (c1 >> 11) & 0x1F;
    let g1 = (c1 >> 5) & 0x3F;
    let b1 = c1 & 0x1F;

    // Expand 565 ref colors to 888
    let r = (r0 as u32 * 527 + 23) >> 6;
    let g = (g0 as u32 * 259 + 33) >> 6;
    let b = (b0 as u32 * 527 + 23) >> 6;
    ref_colors[0] = 0xFF000000 | (b << 16) | (g << 8) | r;

    let r = (r1 as u32 * 527 + 23) >> 6;
    let g = (g1 as u32 * 259 + 33) >> 6;
    let b = (b1 as u32 * 527 + 23) >> 6;
    ref_colors[1] = 0xFF000000 | (b << 16) | (g << 8) | r;

    if c0 > c1 || OPAQUE_MODE {
        // Standard BC1 mode (also BC3 color block uses ONLY this mode)
        // color_2 = 2/3*color_0 + 1/3*color_1
        // color_3 = 1/3*color_0 + 2/3*color_1
        let r = ((2 * r0 as u32 + r1 as u32) * 351 + 61) >> 7;
        let g = ((2 * g0 as u32 + g1 as u32) * 2763 + 1039) >> 11;
        let b = ((2 * b0 as u32 + b1 as u32) * 351 + 61) >> 7;
        ref_colors[2] = 0xFF000000 | (b << 16) | (g << 8) | r;

        let r = ((r0 as u32 + r1 as u32 * 2) * 351 + 61) >> 7;
        let g = ((g0 as u32 + g1 as u32 * 2) * 2763 + 1039) >> 11;
        let b = ((b0 as u32 + b1 as u32 * 2) * 351 + 61) >> 7;
        ref_colors[3] = 0xFF000000 | (b << 16) | (g << 8) | r;
    } else {
        // Quite rare BC1A mode
        // color_2 = 1/2*color_0 + 1/2*color_1
        // color_3 = 0
        let r = ((r0 as u32 + r1 as u32) * 1053 + 125) >> 8;
        let g = ((g0 as u32 + g1 as u32) * 4145 + 1019) >> 11;
        let b = ((b0 as u32 + b1 as u32) * 1053 + 125) >> 8;
        ref_colors[2] = 0xFF000000 | (b << 16) | (g << 8) | r;
        ref_colors[3] = 0x00000000;
    }

    let mut color_indices = u32::from_le_bytes([
        compressed_block[4],
        compressed_block[5],
        compressed_block[6],
        compressed_block[7],
    ]);

    // Fill out the decompressed color block
    for i in 0..4 {
        for j in 0..4 {
            let idx = color_indices & 0x03;
            let offset = j * 4;
            let color = ref_colors[idx as usize];

            decompressed_block[i * destination_pitch + offset..][..4]
                .copy_from_slice(&color.to_le_bytes());

            color_indices >>= 2;
        }
    }
}

/// Decodes a BC2/DXT3 alpha block (sharp transitions)
#[inline(always)]
fn decode_sharp_alpha_block(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    let alpha: [u16; 4] = [
        u16::from_le_bytes([compressed_block[0], compressed_block[1]]),
        u16::from_le_bytes([compressed_block[2], compressed_block[3]]),
        u16::from_le_bytes([compressed_block[4], compressed_block[5]]),
        u16::from_le_bytes([compressed_block[6], compressed_block[7]]),
    ];

    for i in 0..4 {
        for j in 0..4 {
            let alpha_value = ((alpha[i] >> (4 * j)) & 0x0F) as u8;
            decompressed_block[i * destination_pitch + j * 4 + 3] = alpha_value * 17;
        }
    }
}

/// Decodes a BC2/DXT3 alpha block (smooth transitions)
#[inline(always)]
#[rustfmt::skip]
fn decode_smooth_alpha_block<const PIXEL_SIZE: usize>(
    compressed_block: &[u8],
    decompressed_block: &mut [u8],
    destination_pitch: usize,
) {
    let block = u64::from_le_bytes(compressed_block[0..8].try_into().unwrap());

    let mut alpha = [0u8; 8];
    alpha[0] = (block & 0xFF) as u8;
    alpha[1] = ((block >> 8) & 0xFF) as u8;

    if alpha[0] > alpha[1] {
        // 6 interpolated alpha values
        alpha[2] = ((6 * alpha[0] as u16 +     alpha[1] as u16) / 7) as u8;   /* 6/7*alpha_0 + 1/7*alpha_1 */
        alpha[3] = ((5 * alpha[0] as u16 + 2 * alpha[1] as u16) / 7) as u8;   /* 5/7*alpha_0 + 2/7*alpha_1 */
        alpha[4] = ((4 * alpha[0] as u16 + 3 * alpha[1] as u16) / 7) as u8;   /* 4/7*alpha_0 + 3/7*alpha_1 */
        alpha[5] = ((3 * alpha[0] as u16 + 4 * alpha[1] as u16) / 7) as u8;   /* 3/7*alpha_0 + 4/7*alpha_1 */
        alpha[6] = ((2 * alpha[0] as u16 + 5 * alpha[1] as u16) / 7) as u8;   /* 2/7*alpha_0 + 5/7*alpha_1 */
        alpha[7] = ((    alpha[0] as u16 + 6 * alpha[1] as u16) / 7) as u8;   /* 1/7*alpha_0 + 6/7*alpha_1 */
    } else {
        // 4 interpolated alpha values
        alpha[2] = ((4 * alpha[0] as u16 +     alpha[1] as u16) / 5) as u8;   /* 4/5*alpha_0 + 1/5*alpha_1 */
        alpha[3] = ((3 * alpha[0] as u16 + 2 * alpha[1] as u16) / 5) as u8;   /* 3/5*alpha_0 + 2/5*alpha_1 */
        alpha[4] = ((2 * alpha[0] as u16 + 3 * alpha[1] as u16) / 5) as u8;   /* 2/5*alpha_0 + 3/5*alpha_1 */
        alpha[5] = ((    alpha[0] as u16 + 4 * alpha[1] as u16) / 5) as u8;   /* 1/5*alpha_0 + 4/5*alpha_1 */
        alpha[6] = 0x00;
        alpha[7] = 0xFF;
    }

    let mut indices = block >> 16;

    for i in 0..4 {
        for j in 0..4 {
            decompressed_block[i * destination_pitch + j * PIXEL_SIZE] = alpha[(indices & 0x07) as usize];
            indices >>= 3;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{BC1Decoder, BC2Decoder, BC3Decoder, BlockDecoder};

    fn test_block<Decoder: BlockDecoder>(
        compressed_block: &[u8],
        expected_output: &[u8],
        name: &str,
    ) {
        let mut decoded = [0u8; 64];
        let pitch = 16;
        Decoder::decode_block(compressed_block, &mut decoded, pitch);

        for y in 0..4 {
            let start = y * pitch;
            let end = start + pitch;
            assert_eq!(
                &decoded[start..end],
                &expected_output[start..end],
                "{}: Mismatch at row {}",
                name,
                y
            );
        }
    }

    #[test]
    fn test_bc1_block_black() {
        let compressed_block = [0u8; 8];
        let expected_output = [
            0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF,
            0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF,
            0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF,
            0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF,
        ];
        test_block::<BC1Decoder>(&compressed_block, &expected_output, "Black block");
    }

    #[test]
    fn test_bc1_block_red() {
        let compressed_block = [0x00, 0xF8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let expected_output = [
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF,
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF,
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF,
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF,
        ];
        test_block::<BC1Decoder>(&compressed_block, &expected_output, "Red block");
    }

    #[test]
    fn test_bc1_block_gradient() {
        let compressed_block = [0x00, 0xF8, 0xE0, 0x07, 0x55, 0x55, 0x55, 0x55];
        let expected_output = [
            0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF,
            0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF,
            0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF,
            0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF,
        ];
        test_block::<BC1Decoder>(&compressed_block, &expected_output, "Gradient block");
    }

    #[test]
    fn test_bc2_alpha_gradient() {
        let compressed_block = [
            0x10, 0x32, 0x54, 0x76, 0x98, 0xBA, 0xDC, 0xFE, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let expected_output = [
            0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x11, 0xFF, 0x0, 0x0, 0x22, 0xFF, 0x0, 0x0, 0x33,
            0xFF, 0x0, 0x0, 0x44, 0xFF, 0x0, 0x0, 0x55, 0xFF, 0x0, 0x0, 0x66, 0xFF, 0x0, 0x0, 0x77,
            0xFF, 0x0, 0x0, 0x88, 0xFF, 0x0, 0x0, 0x99, 0xFF, 0x0, 0x0, 0xAA, 0xFF, 0x0, 0x0, 0xBB,
            0xFF, 0x0, 0x0, 0xCC, 0xFF, 0x0, 0x0, 0xDD, 0xFF, 0x0, 0x0, 0xEE, 0xFF, 0x0, 0x0, 0xFF,
        ];
        test_block::<BC2Decoder>(&compressed_block, &expected_output, "Alpha gradient");
    }

    #[test]
    fn test_bc2_alpha_half_transparent() {
        let compressed_block = [
            0x77, 0x77, 0x77, 0x77, 0x77, 0x77, 0x77, 0x77, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let expected_output = [
            0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77,
            0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77,
            0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77,
            0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77, 0xFF, 0x0, 0x0, 0x77,
        ];
        test_block::<BC2Decoder>(&compressed_block, &expected_output, "Half transparent");
    }

    #[test]
    fn test_bc3_solid_black() {
        let compressed_block = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let expected_output = [
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
        ];
        test_block::<BC3Decoder>(
            &compressed_block,
            &expected_output,
            "Solid black with full alpha",
        );
    }

    #[test]
    fn test_bc3_transparent_red() {
        let compressed_block = [
            0x00, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let expected_output = [
            0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0,
            0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0,
            0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0,
            0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0, 0xFF, 0x0, 0x0, 0x0,
        ];
        test_block::<BC3Decoder>(&compressed_block, &expected_output, "Transparent red");
    }

    #[test]
    fn test_bc3_alpha_gradient() {
        let compressed_block = [
            0x00, 0xFF, 0xFF, 0xFF, 0x55, 0x55, 0x55, 0x55, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let expected_output = [
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF,
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0x66, 0xFF, 0x0, 0x0, 0xCC, 0xFF, 0x0, 0x0, 0x33,
            0xFF, 0x0, 0x0, 0xCC, 0xFF, 0x0, 0x0, 0x33, 0xFF, 0x0, 0x0, 0xCC, 0xFF, 0x0, 0x0, 0x33,
            0xFF, 0x0, 0x0, 0xCC, 0xFF, 0x0, 0x0, 0x33, 0xFF, 0x0, 0x0, 0xCC, 0xFF, 0x0, 0x0, 0x33,
        ];
        test_block::<BC3Decoder>(
            &compressed_block,
            &expected_output,
            "Red with alpha gradient",
        );
    }

    #[test]
    fn test_bc3_color_alpha_gradient() {
        let compressed_block = [
            0x00, 0xFF, 0xFF, 0xFF, 0x55, 0x55, 0x55, 0x55, 0x00, 0xF8, 0xE0, 0x07, 0x55, 0x55,
            0x55, 0x55,
        ];
        let expected_output = [
            0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF,
            0x0, 0xFF, 0x0, 0xFF, 0x0, 0xFF, 0x0, 0x66, 0x0, 0xFF, 0x0, 0xCC, 0x0, 0xFF, 0x0, 0x33,
            0x0, 0xFF, 0x0, 0xCC, 0x0, 0xFF, 0x0, 0x33, 0x0, 0xFF, 0x0, 0xCC, 0x0, 0xFF, 0x0, 0x33,
            0x0, 0xFF, 0x0, 0xCC, 0x0, 0xFF, 0x0, 0x33, 0x0, 0xFF, 0x0, 0xCC, 0x0, 0xFF, 0x0, 0x33,
        ];
        test_block::<BC3Decoder>(
            &compressed_block,
            &expected_output,
            "Color and alpha gradients",
        );
    }

    #[test]
    fn test_bc3_semi_transparent() {
        let compressed_block = [
            0x80, 0x80, 0xFF, 0xFF, 0xAA, 0xAA, 0xAA, 0xAA, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let expected_output = [
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0xFF,
            0xFF, 0x0, 0x0, 0xFF, 0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80,
            0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80,
            0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80, 0xFF, 0x0, 0x0, 0x80,
        ];
        test_block::<BC3Decoder>(&compressed_block, &expected_output, "Semi-transparent red");
    }

    // TODO: NHA Test BC4 and BC5

    fn create_test_data(decompressed_block: &[u8]) {
        let mut output = String::from("let expected_output = [\n    ");
        for (i, &byte) in decompressed_block.iter().enumerate() {
            if i > 0 && i % 16 == 0 {
                output.push_str(",\n    ");
            } else if i > 0 {
                output.push_str(", ");
            }
            output.push_str(&format!("0x{:x}", byte));
        }
        output.push_str("\n];");

        println!("{}", output);
    }
}
