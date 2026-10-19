//! The temporary `.part-` files that downloads and imports write beside their
//! result, and the ones that a process which died left behind.

use std::fs::DirEntry;
use std::time::Duration;

/// How long a temporary file must stay unchanged before it counts as left
/// behind. A running download writes to its file many times a second and
/// gives up after 30 minutes, and an import takes seconds.
const STALE_AFTER: Duration = Duration::from_secs(60 * 60);

/// Whether `entry` has not changed for an hour, so no running download or
/// import can still be writing it.
pub(crate) fn is_stale(entry: &DirEntry) -> bool {
    entry
        .metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age > STALE_AFTER)
}
