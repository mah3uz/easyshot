use es_model::HistoryEntry;
use uuid::Uuid;

/// Ids beyond `limit` of the newest-first ordering; a limit of 0 keeps everything.
pub fn excess(entries: &[HistoryEntry], limit: usize) -> Vec<Uuid> {
    if limit == 0 {
        return Vec::new();
    }
    let mut ordered: Vec<&HistoryEntry> = entries.iter().collect();
    ordered.sort_by_key(|e| std::cmp::Reverse(e.created_at));
    ordered.iter().skip(limit).map(|e| e.id).collect()
}
