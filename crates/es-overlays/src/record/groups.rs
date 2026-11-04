use es_model::platform::WindowInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowGroup {
    /// None for windows whose workspace the compositor does not report.
    pub workspace: Option<String>,
    /// Some window of the group is on an output now.
    pub shown: bool,
    /// Indices into the list that was grouped, in list order.
    pub windows: Vec<usize>,
}

/// Groups by workspace: unplaced windows first, then workspaces an output shows, then the rest by number or name.
pub fn window_groups(windows: &[WindowInfo]) -> Vec<WindowGroup> {
    let mut groups: Vec<WindowGroup> = Vec::new();
    for (i, w) in windows.iter().enumerate() {
        let shown = w.logical.is_some();
        match groups.iter_mut().find(|g| g.workspace == w.workspace) {
            Some(g) => {
                g.shown |= shown;
                g.windows.push(i);
            }
            None => groups.push(WindowGroup {
                workspace: w.workspace.clone(),
                shown,
                windows: vec![i],
            }),
        }
    }
    groups.sort_by_cached_key(|g| {
        let name = g.workspace.clone().unwrap_or_default();
        let number = name.parse::<i64>().ok();
        (
            g.workspace.is_some(),
            !g.shown,
            number.is_none(),
            number,
            name,
        )
    });
    groups
}
