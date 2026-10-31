//! 相似图：dHash（9x8 灰度相邻比较）+ 汉明距离分组。
//!
//! 使用 `image` 0.25 直接解码，避免 `img_hash` 绑定旧版 image（0.23）造成双版本问题。

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use image::imageops::FilterType;
use rayon::prelude::*;

use crate::models::{MediaFile, MediaKind};

pub fn dhash(path: &Path) -> anyhow::Result<u64> {
    let img = image::open(path)?;
    let small = img.resize_exact(9, 8, FilterType::Triangle).to_luma8();

    let mut hash = 0u64;
    for y in 0..8u32 {
        for x in 0..8u32 {
            let left = small.get_pixel(x, y)[0];
            let right = small.get_pixel(x + 1, y)[0];
            if left > right {
                hash |= 1u64 << (y * 8 + x);
            }
        }
    }
    Ok(hash)
}

pub fn assign_image_hashes(files: &mut [MediaFile], cancel: &AtomicBool) {
    files.par_iter_mut().for_each(|f| {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        if f.kind != MediaKind::Photo {
            return;
        }
        if let Ok(hash) = dhash(&f.source_path) {
            f.image_hash = Some(format!("{:016x}", hash));
        }
    });
}

/// 返回相似分组（每组 >= 2 个下标）。当前为 O(n²) 两两比较，后续可换 BK-tree。
pub fn group_similar(files: &[MediaFile], threshold: u32) -> Vec<Vec<usize>> {
    let candidates: Vec<(usize, u64)> = files
        .iter()
        .enumerate()
        .filter_map(|(i, f)| {
            f.image_hash
                .as_deref()
                .and_then(|h| u64::from_str_radix(h, 16).ok())
                .map(|v| (i, v))
        })
        .collect();

    let n = candidates.len();
    let mut parent: Vec<usize> = (0..n).collect();

    for a in 0..n {
        for b in (a + 1)..n {
            if (candidates[a].1 ^ candidates[b].1).count_ones() <= threshold {
                union(&mut parent, a, b);
            }
        }
    }

    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for (idx, (file_idx, _)) in candidates.iter().enumerate() {
        let root = find(&mut parent, idx);
        groups.entry(root).or_default().push(*file_idx);
    }

    let mut out: Vec<Vec<usize>> = groups
        .into_values()
        .filter(|group| group.len() > 1)
        .collect();
    out.sort_by_key(|g| g[0]);
    out
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra != rb {
        parent[rb] = ra;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MediaFile, MediaKind};
    use std::path::PathBuf;

    fn with_hash(i: usize, hash: u64) -> MediaFile {
        MediaFile {
            id: format!("{i}"),
            source_path: PathBuf::from(format!("/x/{i}.jpg")),
            kind: MediaKind::Photo,
            size_bytes: 1,
            ext: "jpg".into(),
            date: None,
            date_source: None,
            resolved: false,
            byte_hash: None,
            image_hash: Some(format!("{hash:016x}")),
        }
    }

    #[test]
    fn groups_hashes_within_threshold() {
        // 0 与 1 相差 2 位；2 与前两者相差很多。
        let files = vec![with_hash(0, 0), with_hash(1, 0b11), with_hash(2, u64::MAX)];

        let groups = group_similar(&files, 2);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec![0, 1]);

        let strict = group_similar(&files, 1);
        assert!(strict.is_empty());
    }

    #[test]
    fn ignores_files_without_hash() {
        let mut a = with_hash(0, 0);
        a.image_hash = None;
        let files = vec![a];
        assert!(group_similar(&files, 8).is_empty());
    }

    #[test]
    fn decodes_images_and_recognizes_near_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.png");
        let b = dir.path().join("b.png");
        let c = dir.path().join("c.png");

        let gradient = image::RgbImage::from_fn(96, 64, |x, y| {
            image::Rgb([(x * 2) as u8, (y * 3) as u8, ((x + y) * 2) as u8])
        });
        gradient.save(&a).unwrap();
        let slightly_changed = image::RgbImage::from_fn(96, 64, |x, y| {
            image::Rgb([
                ((x * 2) as u8).saturating_add(1),
                ((y * 3) as u8).saturating_add(1),
                (((x + y) * 2) as u8).saturating_add(1),
            ])
        });
        slightly_changed.save(&b).unwrap();

        let different = image::RgbImage::from_fn(96, 64, |x, y| {
            if (x / 8 + y / 8) % 2 == 0 {
                image::Rgb([240, 240, 240])
            } else {
                image::Rgb([10, 10, 10])
            }
        });
        different.save(&c).unwrap();

        let hash_a = dhash(&a).unwrap();
        let hash_b = dhash(&b).unwrap();
        let hash_c = dhash(&c).unwrap();
        assert_eq!((hash_a ^ hash_b).count_ones(), 0);
        assert!((hash_a ^ hash_c).count_ones() > 8);
    }
}
