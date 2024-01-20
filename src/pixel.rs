use std::fmt::Write as _;

use crate::color::Color;


#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Pixel {
    pub x: usize,
    pub y: usize,
    pub color: Color,
}

impl Pixel {    
    pub fn to_pixelflut_string(&self, offset_x: usize, offset_y: usize) -> String {
        let x = self.x + offset_x;
        let y = self.y + offset_y;
        // PX(2) + 1 + X(4) + 1 + Y(4) + 1 + C(6) + \n(1) = 19
        let mut result = String::with_capacity(19);
        result.push_str("PX ");
        result.push_str(&x.to_string());
        result.push(' ');
        result.push_str(&y.to_string());
        result.push(' ');
        write!(result, "{:02x}", self.color.r).unwrap();
        write!(result, "{:02x}", self.color.g).unwrap();
        write!(result, "{:02x}", self.color.b).unwrap();
        result.push('\n');
        result
    }
}