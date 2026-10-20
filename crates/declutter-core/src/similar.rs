//! Similar-image review (new in 1.0). Owner: core-C. Perceptual hash (e.g. `image_hasher` dHash/pHash) over
//! common image types, grouped by Hamming distance. Never preselected; same keep-one rule as duplicates (the user
//! must keep at least one per group); Recycle Bin only. Exact duplicates (identical bytes) belong to `dupes`, not here.
//! Must be cancellable, report `Phase::Similar` progress, skip online-only and hidden files (already excluded by
//! the walk), and never panic on a corrupt image (skip it).

use crate::ctx::Ctx;
use crate::model::SimilarGroup;
use crate::walk::Entry;

/// `max_distance`: Hamming distance threshold (default 6 of 64 bits is a sensible start; document the choice).
pub fn find_similar(ctx: &Ctx, entries: &[Entry], max_distance: u32) -> Vec<SimilarGroup> { todo!("core-C") }
