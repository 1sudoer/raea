//! A video's frames for the fast pass (docs/index.feature IX2): one for each `EVERY` seconds, at most `FRAMES` spread
//! over the whole video, each the keyframe nearest its moment so nothing between keyframes is decoded. The first is the
//! video's thumbnail. Each frame is a row with its moment, so a hit says where in the video it is.

use crate::apple::{SIDE, squash, write_jpeg};
use objc2_av_foundation::{AVAssetImageGenerator, AVURLAsset};
use objc2_core_foundation::{CFRetained, CGSize};
use objc2_core_graphics::CGImage;
use objc2_core_media::{CMTime, kCMTimePositiveInfinity};
use objc2_core_video::CVPixelBuffer;
use objc2_foundation::{NSString, NSURL};
use std::path::Path;

/// seconds of video for each frame, and the most frames a video gives
pub const EVERY: f64 = 4.0;
pub const FRAMES: usize = 16;

/// The moments to take: the middle of each stretch, the stretches `EVERY` seconds long, or longer so a long video
/// gives `FRAMES`. A video whose length can't be read gives its first frame.
pub fn moments(dur: f64) -> Vec<f64> {
    if dur.is_nan() || dur <= 0.0 || dur.is_infinite() {
        return vec![0.0];
    }
    let n = ((dur / EVERY).ceil() as usize).clamp(1, FRAMES);
    let step = dur / n as f64;
    (0..n).map(|i| step * (i as f64 + 0.5)).collect()
}

/// A video's frames squashed for the model, each with its moment in seconds, and the size of its small copy
pub struct Frames {
    pub px: Vec<(CFRetained<CVPixelBuffer>, f64)>,
    pub w: u32,
    pub h: u32,
}

pub fn video(path: &Path, thumb: Option<&Path>) -> Result<Frames, String> {
    objc2::rc::autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let asset = unsafe { AVURLAsset::URLAssetWithURL_options(&url, None) };
        let dur = unsafe { asset.duration().seconds() };
        let generator = unsafe { AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset) };
        unsafe {
            generator.setAppliesPreferredTrackTransform(true);
            generator.setMaximumSize(CGSize::new(SIDE as f64, SIDE as f64));
            // the nearest keyframe, however far: the fast pass never decodes between keyframes
            generator.setRequestedTimeToleranceBefore(kCMTimePositiveInfinity);
            generator.setRequestedTimeToleranceAfter(kCMTimePositiveInfinity);
        }
        let mut f = Frames { px: Vec::new(), w: 0, h: 0 };
        let (mut last, mut why) = (f64::NAN, None);
        for t in moments(dur) {
            let want = unsafe { CMTime::with_seconds(t, 600) };
            let mut got = want;
            #[allow(deprecated)]
            let img = match unsafe { generator.copyCGImageAtTime_actualTime_error(want, &mut got) } {
                Ok(img) => img,
                Err(e) => {
                    why = Some(e.localizedDescription().to_string());
                    continue;
                }
            };
            let at = unsafe { got.seconds() };
            // keyframes further apart than the stretches give the same one again
            if at == last {
                continue;
            }
            last = at;
            if f.px.is_empty() {
                (f.w, f.h) = (CGImage::width(Some(&img)) as u32, CGImage::height(Some(&img)) as u32);
                if let Some(to) = thumb {
                    let _ = write_jpeg(&img, to);
                }
            }
            f.px.push((squash(&img)?, at));
        }
        if f.px.is_empty() {
            return Err(why.unwrap_or_else(|| "no frames in it".into()));
        }
        Ok(f)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_for_each_four_seconds_up_to_sixteen_spread_over_the_whole() {
        assert_eq!(moments(3.0), vec![1.5]);
        assert_eq!(moments(10.0).len(), 3);
        let long = moments(3600.0);
        assert_eq!(long.len(), FRAMES);
        assert!(long[0] > 100.0 && *long.last().unwrap() > 3400.0);
        assert_eq!(moments(f64::NAN), vec![0.0]);
    }
}
