use crate::Color;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Pixel {
    pub x: usize,
    pub y: usize,
    pub color: Color,
}

#[inline]
pub fn pixels_to_cmds(pixels: &[Pixel], offset_x: usize, offset_y: usize) -> Vec<u8> {
    let mut result: Vec<u8> = Vec::with_capacity(pixels.len() * 8);
    for pixel in pixels {        
        let x = pixel.x + offset_x;
        let y = pixel.y + offset_y;
        
        result.push(0xB0); // PX command

        result.push((x >> 8) as u8);
        result.push((x & 0xFF) as u8);
        result.push((y >> 8) as u8);
        result.push((y & 0xFF) as u8);
        
        result.push(pixel.color.r);
        result.push(pixel.color.g);
        result.push(pixel.color.b);
    }
    result
}

