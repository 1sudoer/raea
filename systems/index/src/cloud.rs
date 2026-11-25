//! Files kept only in iCloud (docs/index.feature IX7). A file whose bytes are only in the cloud (SF_DATALESS) lists at
//! its full size, but reading it downloads the original, so the fast pass takes its picture from Quick Look instead:
//! the thumbnail iCloud keeps of it, at 256 px, which leaves the file in the cloud (measured 11-15: 40 of 40 still
//! dataless after). iCloud hands them over about one at a time (1.7 a second the first time, then from Quick Look's
//! cache), so they go in after the local files. Other providers (Google Drive, Dropbox under ~/Library/CloudStorage)
//! aren't known to answer Quick Look without a download, so their cloud-only files stay out and are counted.

use crate::apple::{SIDE, squash, write_jpeg};
use crate::walk::{cloud_only, in_icloud};
use block2::RcBlock;
use objc2::AllocAnyThread;
use objc2_core_foundation::{CFRetained, CGSize};
use objc2_core_video::CVPixelBuffer;
use objc2_foundation::{NSError, NSString, NSURL};
use objc2_quick_look_thumbnailing::{QLThumbnailGenerationRequest, QLThumbnailGenerationRequestRepresentationTypes, QLThumbnailGenerator, QLThumbnailRepresentation};
use std::path::Path;
use std::time::Duration;

/// Whether the fast pass takes this file's picture from Quick Look: only in the cloud, and in iCloud
pub fn wants(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| cloud_only(&m)) && in_icloud(path)
}

/// A cloud-only file's picture from Quick Look at `SIDE`, its thumbnail written to `thumb`; waits for iCloud's answer
/// up to a minute
pub fn picture(path: &Path, thumb: Option<&Path>) -> Result<(CFRetained<CVPixelBuffer>, u32, u32), String> {
    objc2::rc::autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let side = CGSize::new(SIDE as f64, SIDE as f64);
        let req = unsafe { QLThumbnailGenerationRequest::initWithFileAtURL_size_scale_representationTypes(QLThumbnailGenerationRequest::alloc(), &url, side, 1.0, QLThumbnailGenerationRequestRepresentationTypes::Thumbnail) };
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let answer = RcBlock::new(move |rep: *mut QLThumbnailRepresentation, err: *mut NSError| {
            let got = match unsafe { rep.as_ref() } {
                Some(rep) => Ok(unsafe { rep.CGImage() }),
                None => Err(unsafe { err.as_ref() }.map(|e| e.localizedDescription().to_string()).unwrap_or_else(|| "no thumbnail".into())),
            };
            let _ = tx.send(got);
        });
        unsafe { QLThumbnailGenerator::sharedGenerator().generateBestRepresentationForRequest_completionHandler(&req, &answer) };
        let cg = rx.recv_timeout(Duration::from_secs(60)).map_err(|_| "iCloud didn't answer for its thumbnail".to_string())?.map_err(|e| format!("in iCloud, and Quick Look has no thumbnail of it: {e}"))?;
        let (w, h) = (objc2_core_graphics::CGImage::width(Some(&cg)) as u32, objc2_core_graphics::CGImage::height(Some(&cg)) as u32);
        if let Some(t) = thumb {
            let _ = write_jpeg(&cg, t);
        }
        Ok((squash(&cg)?, w, h))
    })
}
