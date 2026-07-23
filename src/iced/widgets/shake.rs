use iced::Padding;

#[derive(Debug, Clone, Default)]
pub struct Shake {
    offset: f32,
}

impl Shake {
    pub fn new() -> Self {
        Self { offset: 0.0 }
    }

    pub fn trigger(&mut self) {
        self.offset = 10.0;
    }

    pub fn tick(&mut self) {
        if self.offset.abs() > 0.1 {
            self.offset = -self.offset * 0.8;
            if self.offset.abs() < 0.5 {
                self.offset = 0.0;
            }
        }
    }

    pub fn apply(&self) -> Padding {
        if self.offset != 0.0 {
            Padding {
                left: self.offset.max(0.0),
                right: (-self.offset).max(0.0),
                top: 0.0,
                bottom: 0.0,
            }
        } else {
            Padding::default()
        }
    }

    pub fn is_shaking(&self) -> bool {
        self.offset.abs() > 0.0
    }
}
