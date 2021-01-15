use std::cmp;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn intersection(&self, other: Rect) -> Option<Rect> {
        let ix = cmp::max(self.x, other.x);
        let iy = cmp::max(self.y, other.y);
        let iw = cmp::min(self.x + self.w, other.x + other.w) - ix;
        let ih = cmp::min(self.y + self.h, other.y + other.h) - iy;

        if iw > 0 && ih > 0 {
            Some(Rect {
                x: ix,
                y: iy,
                w: iw,
                h: ih,
            })
        } else {
            None
        }
    }
}
