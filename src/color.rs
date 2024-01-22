
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Color { pub r: u8, pub g: u8, pub b: u8 }

impl Color {
    #[inline]
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
    /// Converts RGB to YUV
    // https://en.wikipedia.org/wiki/Y%E2%80%B2UV#Conversion_to/from_RGB
    pub fn to_yuv(&self) -> (u8, u8, u8) {
        let r = self.r as f32; let g = self.g as f32; let b = self.b as f32;
        
        let l = r * 0.299    + g * 0.587   + b * 0.114;
        let u = r * -0.14713 - g * 0.28886 + b * 0.436   + 128.0;
        let v = r * 0.615    - g * 0.51499 - b * 0.10001 + 128.0;

        (l as u8, u as u8, v as u8)    
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::*;
    
    #[case(255, 255, 255, 255, 128, 128)]
    #[case(0, 0, 0, 0, 128, 128)]
    fn test_color_to_yuv(r: u8, g: u8, b: u8, y: u8, u: u8, v: u8) {
        let c = Color::new(r, g, b);
        assert_eq!(c.to_yuv(), (y, u, v));
    }    
}