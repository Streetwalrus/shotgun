// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use image::DynamicImage;
use image::GenericImage;
use image::Pixel;
use image::Rgba;
use image::RgbaImage;
use x11::xlib;

pub mod error;
pub mod util;
pub mod xwrap;
use crate::xwrap::Display;
use error::CaptureError;

/// Take a screenshot from the currently active X11 server.
///
/// If you specify the `window_id`, you must make sure that a window with that ID exists.
///
/// If you specify the `window_geometry` it should be parsed by [`xwrap::parse_geometry`](./xwrap/fn.parse_geometry.html)
///
/// Submitting an invalid geometry will yield an [`CaptureError::InvalidGeometry`](./error/enum.CaptureError.html)
pub fn capture(
    window_id: Option<xlib::Window>,
    window_geometry: Option<util::Rect>,
) -> Result<DynamicImage, CaptureError> {
    let display = match Display::open(None) {
        Some(d) => d,
        None => return Err(CaptureError::DisplayOpen),
    };

    let root = display.get_default_root();
    let window = window_id.unwrap_or(root);

    let window_rect = display.get_window_rect(window);
    let sel = match window_geometry {
        Some(geometry) => match geometry.intersection(window_rect) {
            Some(sel) => util::Rect {
                // Selection is relative to the root window (whole screen)
                x: sel.x - window_rect.x,
                y: sel.y - window_rect.y,
                w: sel.w,
                h: sel.h,
            },
            None => {
                return Err(CaptureError::InvalidGeometry);
            }
        },
        None => util::Rect {
            x: 0,
            y: 0,
            w: window_rect.w,
            h: window_rect.h,
        },
    };

    let image = match display.get_image(window, sel, xwrap::ALL_PLANES, xlib::ZPixmap) {
        Some(i) => i,
        None => return Err(CaptureError::FailedToCaptureFromX11),
    };

    let mut image = match image.into_image_buffer() {
        Some(i) => image::DynamicImage::ImageRgba8(i),
        None => return Err(CaptureError::UnableToConvertFramebuffer),
    };

    // When capturing the root window, attempt to mask the off-screen areas
    if window == root {
        match display.get_screen_rects(root) {
            Some(screens) => {
                let screens: Vec<util::Rect> =
                    screens.filter_map(|s| s.intersection(sel)).collect();

                // No point in masking if we're only capturing one screen
                if screens.len() > 1 {
                    let mut masked = RgbaImage::from_pixel(
                        sel.w as u32,
                        sel.h as u32,
                        Rgba::from_channels(0, 0, 0, 0),
                    );

                    for screen in screens {
                        // Subimage is relative to the captured area
                        let sub = util::Rect {
                            x: screen.x - sel.x,
                            y: screen.y - sel.y,
                            w: screen.w,
                            h: screen.h,
                        };

                        let mut sub_src =
                            image.sub_image(sub.x as u32, sub.y as u32, sub.w as u32, sub.h as u32);
                        masked
                            .copy_from(&mut sub_src, sub.x as u32, sub.y as u32)
                            .expect("Failed to copy sub-image");
                    }

                    image = image::DynamicImage::ImageRgba8(masked);
                }
            }
            None => {
                return Err(CaptureError::FailedToEnumerateScreens);
            }
        }
    }

    Ok(image)
}
