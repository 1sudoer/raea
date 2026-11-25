//! Apple Photos as a source (docs/index.feature IX6): every photo and video in the person's library, and so their
//! iCloud Photos and iPhone camera roll, listed through PhotoKit and keyed by the asset's local identifier. The fast
//! pass reads the small copy Photos keeps of each on this Mac (PHImageManager at 256 px, never over the network), so a
//! library whose originals stay in iCloud goes in without downloading them. A video gives its poster. macOS asks the
//! person once; the app's Info.plist says why (NSPhotoLibraryUsageDescription).

use crate::Found;
use crate::apple::{SIDE, squash, write_jpeg};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2_app_kit::NSImage;
use objc2_core_foundation::{CFRetained, CGSize};
use objc2_core_graphics::CGImage;
use objc2_core_video::CVPixelBuffer;
use objc2_foundation::{NSArray, NSDate, NSDictionary, NSString};
use objc2_photos::{
    PHAccessLevel, PHAsset, PHAssetMediaType, PHAssetResource, PHAuthorizationStatus, PHImageContentMode, PHImageManager, PHImageRequestOptions,
    PHImageRequestOptionsDeliveryMode, PHImageRequestOptionsResizeMode, PHPhotoLibrary,
};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

// the source's root, and the start of each of its locators (then the asset's local identifier)
use crate::PHOTOS as ROOT;

/// Whether raea may read the library, asking the person the first time (macOS shows its prompt once)
pub fn access() -> Result<(), String> {
    let level = PHAccessLevel::ReadWrite;
    let mut status = unsafe { PHPhotoLibrary::authorizationStatusForAccessLevel(level) };
    if status == PHAuthorizationStatus::NotDetermined {
        let (tx, rx) = std::sync::mpsc::channel();
        let answer = RcBlock::new(move |s: PHAuthorizationStatus| {
            let _ = tx.send(s);
        });
        unsafe { PHPhotoLibrary::requestAuthorizationForAccessLevel_handler(level, &answer) };
        status = rx.recv().unwrap_or(PHAuthorizationStatus::Denied);
    }
    if status == PHAuthorizationStatus::Authorized || status == PHAuthorizationStatus::Limited {
        return Ok(());
    }
    Err("raea can't read Photos: turn it on in System Settings, Privacy & Security, Photos".into())
}

fn secs(d: Option<Retained<NSDate>>) -> Option<i64> {
    d.map(|d| d.timeIntervalSince1970() as i64)
}

/// One asset as the store lists it: its name is the original file's, as Photos shows it
fn found(a: &PHAsset) -> Option<Found> {
    let kind = match unsafe { a.mediaType() } {
        PHAssetMediaType::Image => "picture",
        PHAssetMediaType::Video => "video",
        _ => return None,
    };
    let taken = secs(unsafe { a.creationDate() });
    let resources = unsafe { PHAssetResource::assetResourcesForAsset(a) };
    let name = resources.firstObject().map(|r| unsafe { r.originalFilename() }.to_string()).unwrap_or_default();
    Some(Found {
        locator: format!("{ROOT}{}", unsafe { a.localIdentifier() }),
        kind: kind.into(),
        name,
        size: 0,
        mtime: secs(unsafe { a.modificationDate() }).or(taken).unwrap_or(0),
        taken,
        w: Some(unsafe { a.pixelWidth() } as u32),
        h: Some(unsafe { a.pixelHeight() } as u32),
        dur: (kind == "video").then(|| unsafe { a.duration() }),
    })
}

/// Every photo and video in the library, `on` called with each `every` as they're read
pub fn list(every: usize, on: &mut dyn FnMut(Vec<Found>)) -> Result<(), String> {
    access()?;
    let all = unsafe { PHAsset::fetchAssetsWithOptions(None) };
    let n = unsafe { all.count() };
    let mut at = 0;
    while at < n {
        let end = (at + every).min(n);
        let batch = objc2::rc::autoreleasepool(|_| (at..end).filter_map(|i| found(&*unsafe { all.objectAtIndex(i) })).collect::<Vec<_>>());
        on(batch);
        at = end;
    }
    Ok(())
}

/// An asset's small copy at `SIDE` from the copies Photos keeps on this Mac, its thumbnail written to `thumb`
pub fn picture(id: &str, thumb: Option<&Path>) -> Result<(CFRetained<CVPixelBuffer>, u32, u32), String> {
    objc2::rc::autoreleasepool(|_| {
        let ids = NSArray::from_retained_slice(&[NSString::from_str(id)]);
        let asset = unsafe { PHAsset::fetchAssetsWithLocalIdentifiers_options(&ids, None).firstObject() }.ok_or("no longer in Photos")?;
        let o = unsafe { PHImageRequestOptions::new() };
        unsafe {
            // one answer, before the call returns; never the network
            o.setSynchronous(true);
            o.setNetworkAccessAllowed(false);
            o.setDeliveryMode(PHImageRequestOptionsDeliveryMode::HighQualityFormat);
            o.setResizeMode(PHImageRequestOptionsResizeMode::Fast);
        }
        let got: Rc<RefCell<Option<Retained<NSImage>>>> = Rc::default();
        let slot = got.clone();
        let take = RcBlock::new(move |img: *mut NSImage, _: *mut NSDictionary| {
            *slot.borrow_mut() = unsafe { Retained::retain(img) };
        });
        let side = CGSize::new(SIDE as f64, SIDE as f64);
        unsafe { PHImageManager::defaultManager().requestImageForAsset_targetSize_contentMode_options_resultHandler(&asset, side, PHImageContentMode::AspectFill, Some(&o), &take) };
        let img = got.borrow_mut().take().ok_or("Photos has no small copy of it on this Mac")?;
        let cg = unsafe { img.CGImageForProposedRect_context_hints(std::ptr::null_mut(), None, None) }.ok_or("no picture in Photos' copy")?;
        let (w, h) = (CGImage::width(Some(&cg)) as u32, CGImage::height(Some(&cg)) as u32);
        if let Some(t) = thumb {
            let _ = write_jpeg(&cg, t);
        }
        Ok((squash(&cg)?, w, h))
    })
}
