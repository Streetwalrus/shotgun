// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::ffi::CString;
use std::mem;
use std::os::raw;
use std::ptr;
use std::slice;

use image::Pixel;
use image::Rgba;
use image::RgbaImage;
use libc;
use x11::xlib;
use x11::xrandr;

use crate::util;

pub const ALL_PLANES: libc::c_ulong = !0;

pub struct Display {
    handle: *mut xlib::Display,
}

pub struct Image {
    handle: *mut xlib::XImage,
}

pub struct ScreenRectIter<'a> {
    dpy: &'a Display,
    res: *mut xrandr::XRRScreenResources,
    crtcs: &'a [xrandr::RRCrtc],
    i: usize,
}

impl Display {
    pub fn open(name: Option<CString>) -> Option<Display> {
        unsafe {
            let name = match name {
                None => ptr::null(),
                Some(cstr) => cstr.as_ptr(),
            };
            let d = xlib::XOpenDisplay(name);

            if d.is_null() {
                return None;
            }

            Some(Display { handle: d })
        }
    }

    pub fn get_default_root(&self) -> xlib::Window {
        unsafe { xlib::XDefaultRootWindow(self.handle) }
    }

    pub fn get_window_rect(&self, window: xlib::Window) -> util::Rect {
        unsafe {
            let mut attrs = mem::MaybeUninit::uninit();
            xlib::XGetWindowAttributes(self.handle, window, attrs.as_mut_ptr());
            let attrs = attrs.assume_init();

            let mut root = 0;
            let mut parent = 0;
            let mut children: *mut xlib::Window = ptr::null_mut();
            let mut nchildren = 0;
            xlib::XQueryTree(
                self.handle,
                window,
                &mut root,
                &mut parent,
                &mut children,
                &mut nchildren,
            );
            if !children.is_null() {
                xlib::XFree(children as *mut raw::c_void);
            }

            let mut x = attrs.x;
            let mut y = attrs.y;

            if parent != 0 {
                let mut child = 0;
                xlib::XTranslateCoordinates(
                    self.handle,
                    parent,
                    root,
                    attrs.x,
                    attrs.y,
                    &mut x,
                    &mut y,
                    &mut child,
                );
            }

            util::Rect {
                x: x,
                y: y,
                w: attrs.width,
                h: attrs.height,
            }
        }
    }

    pub fn get_image(
        &self,
        window: xlib::Window,
        rect: util::Rect,
        plane_mask: libc::c_ulong,
        format: libc::c_int,
    ) -> Option<Image> {
        unsafe {
            let image = xlib::XGetImage(
                self.handle,
                window,
                rect.x,
                rect.y,
                rect.w as libc::c_uint,
                rect.h as libc::c_uint,
                plane_mask,
                format,
            );

            if image.is_null() {
                return None;
            }

            Some(Image::from_raw_ximage(image))
        }
    }

    pub fn get_screen_rects(&self, root: xlib::Window) -> Option<ScreenRectIter<'_>> {
        unsafe {
            let xrr_res = xrandr::XRRGetScreenResourcesCurrent(self.handle, root);

            if xrr_res.is_null() {
                return None;
            }

            Some(ScreenRectIter {
                dpy: &self,
                res: xrr_res,
                crtcs: slice::from_raw_parts((*xrr_res).crtcs, (*xrr_res).ncrtc as usize),
                i: 0,
            })
        }
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        unsafe {
            xlib::XCloseDisplay(self.handle);
        }
    }
}

impl Image {
    pub fn from_raw_ximage(ximage: *mut xlib::XImage) -> Image {
        Image { handle: ximage }
    }

    pub fn into_image_buffer(&self) -> Option<RgbaImage> {
        // Extract values from the XImage into our own scope
        macro_rules! get {
            ($($a:ident),+) => ($(let $a = (*self.handle).$a;)+);
        }
        unsafe {
            get!(
                width,
                height,
                byte_order,
                depth,
                bytes_per_line,
                bits_per_pixel,
                red_mask,
                green_mask,
                blue_mask
            );

            // Pixel size
            let stride = match (depth, bits_per_pixel) {
                (24, 24) => 3,
                (24, 32) | (32, 32) => 4,
                _ => return None,
            };

            // Compute subpixel offsets into each pixel according the the bitmasks X gives us
            // Only 8 bit, byte-aligned values are supported
            // Truncate masks to the lower 32 bits as that is the maximum pixel size
            macro_rules! channel_offset {
                ($mask:expr) => {
                    match (byte_order, $mask & 0xFFFFFFFF) {
                        (0, 0xFF) | (1, 0xFF000000) => 0,
                        (0, 0xFF00) | (1, 0xFF0000) => 1,
                        (0, 0xFF0000) | (1, 0xFF00) => 2,
                        (0, 0xFF000000) | (1, 0xFF) => 3,
                        _ => return None,
                    }
                };
            }
            let red_offset = channel_offset!(red_mask);
            let green_offset = channel_offset!(green_mask);
            let blue_offset = channel_offset!(blue_mask);
            let alpha_offset = channel_offset!(!(red_mask | green_mask | blue_mask));

            // Wrap the pixel buffer into a slice
            let size = (bytes_per_line * height) as usize;
            let data = slice::from_raw_parts((*self.handle).data as *const u8, size);

            // Finally, generate the image object
            Some(RgbaImage::from_fn(width as u32, height as u32, |x, y| {
                macro_rules! subpixel {
                    ($channel_offset:ident) => {
                        data[(y * bytes_per_line as u32 + x * stride as u32 + $channel_offset)
                            as usize]
                    };
                }
                Rgba::from_channels(
                    subpixel!(red_offset),
                    subpixel!(green_offset),
                    subpixel!(blue_offset),
                    // Make the alpha channel fully opaque if none is provided
                    if depth == 24 {
                        0xFF
                    } else {
                        subpixel!(alpha_offset)
                    },
                )
            }))
        }
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe {
            xlib::XDestroyImage(self.handle);
        }
    }
}

impl<'a> Iterator for ScreenRectIter<'a> {
    type Item = util::Rect;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.crtcs.len() {
            return None;
        }

        unsafe {
            // TODO Handle failure here?
            let crtc = xrandr::XRRGetCrtcInfo((*self.dpy).handle, self.res, self.crtcs[self.i]);
            let x = (*crtc).x;
            let y = (*crtc).y;
            let w = (*crtc).width;
            let h = (*crtc).height;
            xrandr::XRRFreeCrtcInfo(crtc);

            self.i += 1;

            //Some((w as i32, h as i32, x as i32, y as i32))
            Some(util::Rect {
                x: x,
                y: y,
                w: w as i32,
                h: h as i32,
            })
        }
    }
}

impl<'a> Drop for ScreenRectIter<'a> {
    fn drop(&mut self) {
        unsafe {
            xrandr::XRRFreeScreenResources(self.res);
        }
    }
}

/// Parse the Area to capture.
///
/// The format is `WxH+X+Y` to fully specify the format of `man 3 XParseGeometry`
/// `[=][<width>{xX}<height>][{+-}<xoffset>{+-}<yoffset>]`
pub fn parse_geometry(g: &str) -> Result<util::Rect, ParseError> {
    let g = CString::new(g).expect("Failed to convert CString");
    let mut x = i32::MIN;
    let mut y = i32::MIN;
    let mut w = u32::MIN;
    let mut h = u32::MIN;

    let ret: i32 = unsafe {
        xlib::XParseGeometry(
            g.as_ptr() as *const raw::c_char,
            &mut x,
            &mut y,
            &mut w,
            &mut h,
        )
    };
    const ALL_VALUES: i32 = xlib::YValue | xlib::XValue | xlib::WidthValue | xlib::HeightValue;
    const WH_VALUES: i32 = xlib::WidthValue | xlib::HeightValue;

    match ret {
        ALL_VALUES => Ok(util::Rect {
            x: x,
            y: y,
            w: w as i32,
            h: h as i32,
        }),
        xlib::NoValue => Err(ParseError::NoValue),
        WH_VALUES => Err(ParseError::MissingXY),
        v => unreachable!("parse_geometry reached unreachable return {:b}", v),
    }
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    NoValue,
    MissingXY,
}

#[cfg(test)]
mod test {
    use super::*;
    use util::Rect;

    #[test]
    fn valid_geometry() {
        let g = "20x30+40+50";
        let expect = Ok(Rect {
            w: 20,
            h: 30,
            x: 40,
            y: 50,
        });
        let res = parse_geometry(g);
        assert_eq!(expect, res);
    }
    #[test]
    fn valid_geometry_leading_equal() {
        let g = "=20x30+40+50";
        let expect = Ok(Rect {
            w: 20,
            h: 30,
            x: 40,
            y: 50,
        });
        let res = parse_geometry(g);
        assert_eq!(expect, res);
    }
    #[test]
    fn partial_geometry() {
        let g = "20x30";
        let expect = Err(ParseError::MissingXY);
        let res = parse_geometry(g);
        assert_eq!(expect, res);
    }
    #[test]
    fn invalid_geometry() {
        let g = "";
        let expect = Err(ParseError::NoValue);
        let res = parse_geometry(g);
        assert_eq!(expect, res);
    }
}
