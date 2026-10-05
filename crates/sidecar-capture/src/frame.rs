use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct CaptureFrame {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
}

impl CaptureFrame {
    pub fn new(rgba: Vec<u8>, width: u32, height: u32) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        Self {
            rgba,
            width,
            height,
            timestamp,
        }
    }

    pub fn byte_len(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }

    pub fn validate(&self) -> bool {
        self.width > 0 && self.height > 0 && self.rgba.len() == self.byte_len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_checks_dimensions() {
        let frame = CaptureFrame::new(vec![0; 4], 1, 1);
        assert!(frame.validate());
        let bad = CaptureFrame::new(vec![0; 3], 1, 1);
        assert!(!bad.validate());
    }
}
