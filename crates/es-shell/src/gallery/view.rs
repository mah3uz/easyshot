use super::files::{FileResult, GalleryFiles, capture_files};
use super::model::{self, Category, Item, MediaKind, SortOrder, Step, ViewMode};
use super::thumbs::{self, Thumbs};
use crate::strings::fl;
use es_app::history::History;
use es_store::Paths;
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, ScrollStrategy,
    Subscription, UniformListScrollHandle, Window,
};
use gpui_component::{
    WindowExt,
    button::ButtonVariant,
    input::{InputEvent, InputState},
};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GalleryEvent {
    Open(PathBuf),
    Edit(PathBuf),
}

pub struct Gallery {
    pub(super) history: Entity<History>,
    pub(super) paths: Paths,
    pub(super) files: Arc<dyn GalleryFiles>,
    pub(super) items: Vec<Item>,
    pub(super) shown: Vec<Item>,
    pub(super) category: Category,
    pub(super) sort: SortOrder,
    pub(super) view: ViewMode,
    pub(super) query: String,
    pub(super) selected: Option<Uuid>,
    pub(super) columns: usize,
    pub(super) thumbs: Thumbs,
    pub(super) error: Option<String>,
    pub(super) search: Entity<InputState>,
    pub(super) focus: FocusHandle,
    pub(super) grid_scroll: UniformListScrollHandle,
    pub(super) list_scroll: UniformListScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<GalleryEvent> for Gallery {}

impl Focusable for Gallery {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Gallery {
    pub fn new(
        history: Entity<History>,
        paths: Paths,
        files: Arc<dyn GalleryFiles>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder(fl!("gallery-search-prompt")));
        let on_search = cx.subscribe(&search, |this: &mut Self, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.query = input.read(cx).value().to_string();
                this.refilter(cx);
            }
        });
        let on_history = cx.observe(&history, |this, _, cx| this.sync(cx));
        let mut gallery = Self {
            history,
            paths,
            files,
            items: Vec::new(),
            shown: Vec::new(),
            category: Category::All,
            sort: SortOrder::default(),
            view: ViewMode::default(),
            query: String::new(),
            selected: None,
            columns: 1,
            thumbs: Thumbs::default(),
            error: None,
            search,
            focus: cx.focus_handle(),
            grid_scroll: UniformListScrollHandle::new(),
            list_scroll: UniformListScrollHandle::new(),
            _subscriptions: vec![on_search, on_history],
        };
        gallery.sync(cx);
        gallery
    }

    pub fn shown(&self) -> &[Item] {
        &self.shown
    }

    pub fn count(&self, category: Category) -> usize {
        model::count(&self.items, category)
    }

    pub fn category(&self) -> Category {
        self.category
    }

    pub fn sort(&self) -> SortOrder {
        self.sort
    }

    pub fn view_mode(&self) -> ViewMode {
        self.view
    }

    pub fn selected(&self) -> Option<Uuid> {
        self.selected
    }

    pub fn selected_index(&self) -> Option<usize> {
        let id = self.selected?;
        self.shown.iter().position(|i| i.id == id)
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn search_field(&self) -> Entity<InputState> {
        self.search.clone()
    }

    pub fn thumbnails_ready(&self) -> usize {
        self.thumbs.ready()
    }

    /// Drops entries whose file is gone and retries thumbnails that failed.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.thumbs = Thumbs::default();
        self.sync(cx);
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        self.items = model::local(self.history.read(cx).entries());
        let items = &self.items;
        self.thumbs
            .retain(|file| items.iter().any(|i| i.file.as_path() == file));
        self.refilter(cx);
    }

    fn refilter(&mut self, cx: &mut Context<Self>) {
        self.shown = model::visible(&self.items, self.category, &self.query, self.sort);
        if self.selected_index().is_none() {
            self.selected = None;
        }
        cx.notify();
    }

    pub fn set_category(&mut self, category: Category, cx: &mut Context<Self>) {
        self.category = category;
        self.refilter(cx);
    }

    pub fn set_sort(&mut self, sort: SortOrder, cx: &mut Context<Self>) {
        self.sort = sort;
        self.refilter(cx);
    }

    pub fn set_view(&mut self, view: ViewMode, cx: &mut Context<Self>) {
        self.view = view;
        self.reveal_selection();
        cx.notify();
    }

    pub fn set_query(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |input, cx| {
            input.set_value(query.to_owned(), window, cx)
        });
        self.query = query.to_owned();
        self.refilter(cx);
    }

    pub fn select(&mut self, id: Option<Uuid>, cx: &mut Context<Self>) {
        self.selected = id.filter(|id| self.shown.iter().any(|i| i.id == *id));
        self.reveal_selection();
        cx.notify();
    }

    pub fn step(&mut self, step: Step, cx: &mut Context<Self>) {
        let columns = match self.view {
            ViewMode::Icons => self.columns,
            ViewMode::List => 1,
        };
        let next = model::step(self.selected_index(), step, columns, self.shown.len());
        self.select(next.map(|i| self.shown[i].id), cx);
    }

    fn reveal_selection(&self) {
        let Some(index) = self.selected_index() else {
            return;
        };
        match self.view {
            ViewMode::Icons => self
                .grid_scroll
                .scroll_to_item(index / self.columns.max(1), ScrollStrategy::Nearest),
            ViewMode::List => self
                .list_scroll
                .scroll_to_item(index, ScrollStrategy::Nearest),
        }
    }

    pub(super) fn item(&self, id: Uuid) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn open(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(file) = self.live_file(id, cx) {
            cx.emit(GalleryEvent::Open(file));
        }
    }

    pub fn edit(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let Some(item) = self.item(id) else { return };
        if item.kind != MediaKind::Screenshot {
            return;
        }
        if let Some(file) = self.live_file(id, cx) {
            cx.emit(GalleryEvent::Edit(file));
        }
    }

    fn live_file(&mut self, id: Uuid, cx: &mut Context<Self>) -> Option<PathBuf> {
        let file = self.item(id)?.file.clone();
        if file.is_file() {
            return Some(file);
        }
        self.error = Some(fl!("gallery-error-missing"));
        cx.notify();
        None
    }

    pub fn show_in_folder(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let Some(file) = self.live_file(id, cx) else {
            return;
        };
        let request = self.files.show_in_folder(file);
        cx.spawn(async move |this, cx| {
            if let Err(error) = request.await {
                tracing::warn!(%error, "cannot show the capture in its folder");
                let _ = this.update(cx, |this, cx| {
                    this.error = Some(error);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Moves the capture, its sidecar and its base to the Trash, then drops it from the index.
    pub fn trash(&mut self, id: Uuid, cx: &mut Context<Self>) -> FileResult {
        let Some(item) = self.item(id).cloned() else {
            return Ok(());
        };
        let result = capture_files(&item.file)
            .iter()
            .try_for_each(|file| self.files.trash(file))
            .and_then(|()| {
                self.history
                    .update(cx, |history, cx| history.remove(id, cx))
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            });
        if let Err(error) = &result {
            tracing::warn!(%error, "cannot move the capture to the Trash");
            self.error = Some(fl!("gallery-banner-error", error = error.clone()));
            self.sync(cx);
        }
        result
    }

    pub fn dismiss_error(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        cx.notify();
    }

    pub(super) fn confirm_trash(&self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.item(id) else { return };
        let title = item.name.clone();
        let this = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let this = this.clone();
            dialog
                .title(fl!("gallery-trash-alert-title"))
                .description(fl!("gallery-trash-alert-message", title = title.clone()))
                .ok_text(fl!("gallery-trash-confirm"))
                .ok_variant(ButtonVariant::Danger)
                .cancel_text(fl!("gallery-cancel"))
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let _ = this.update(cx, |this, cx| this.trash(id, cx));
                    true
                })
        });
    }

    /// Starts loading the thumbnails of items about to be drawn; each loads once.
    pub(super) fn request_thumbs(&mut self, range: std::ops::Range<usize>, cx: &mut Context<Self>) {
        let wanted: Vec<PathBuf> = self.shown
            [range.start.min(self.shown.len())..range.end.min(self.shown.len())]
            .iter()
            .filter(|i| i.kind == MediaKind::Screenshot)
            .map(|i| i.file.clone())
            .collect();
        for file in wanted {
            if !self.thumbs.claim(&file) {
                continue;
            }
            let paths = self.paths.clone();
            let source = file.clone();
            let load = cx.background_spawn(async move { thumbs::load(&paths, &source) });
            cx.spawn(async move |this, cx| {
                let image = load.await;
                let _ = this.update(cx, |this, cx| {
                    this.thumbs.finish(file, image);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    pub(super) fn set_columns(&mut self, columns: usize) {
        self.columns = columns.max(1);
    }
}
