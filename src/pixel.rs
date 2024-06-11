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
        let x = (pixel.x + offset_x) as u16; 
        let y = (pixel.y + offset_y) as u16;
        
        result.push(0xB0); // PX command

        result.extend(&x.to_be_bytes());
        result.extend(&y.to_be_bytes());
        
        result.push(pixel.color.r);
        result.push(pixel.color.g);
        result.push(pixel.color.b);
    }
    result
}

