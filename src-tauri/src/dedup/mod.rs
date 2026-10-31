//! M3 去重编排：先字节级（size + blake3），再相似图（dHash + 汉明距离）。
//!
//! 返回的分组是 `files` 的下标，供 M4 `plan.rs` 决定「保留哪一个、隔离哪些」。

pub mod byte;
pub mod perceptual;

use std::sync::atomic::AtomicBool;

use crate::models::{DedupOptions, MediaFile};

#[derive(Debug, Clone, Default)]
pub struct DedupResult {
    pub byte_groups: Vec<Vec<usize>>,
    pub similar_groups: Vec<Vec<usize>>,
}

pub fn run(files: &mut [MediaFile], options: &DedupOptions, cancel: &AtomicBool) -> DedupResult {
    let mut result = DedupResult::default();

    if options.byte {
        byte::assign_byte_hashes(files, cancel);
        result.byte_groups = byte::group_duplicates(files);
    }

    if let Some(cfg) = &options.similar_images {
        perceptual::assign_image_hashes(files, cancel);
        result.similar_groups = perceptual::group_similar(files, cfg.hamming_threshold);
    }

    result
}
