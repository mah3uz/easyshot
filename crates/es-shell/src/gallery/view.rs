use super::files::{FileResult, GalleryFiles, capture_files};
use super::model::{self, Category, Item, MediaKind, Section, SortOrder, Step, ViewMode};
use super::shared;
use super::thumbs::{self, Thumbs};
use crate::settings::ClipboardSink;
use crate::strings::fl;
use es_app::history::History;
use es_app::settings_model::SettingsModel;
use es_app::share::{ShareDeleted, Shares, SharesChanged};
use es_store::Paths;
use gpui::{
    App, AppContext, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable, Pixels,
    ScrollStrategy, Subscription, UniformListScrollHandle, Window,
};
use gpui_component::{
    WindowExt,
    button::ButtonVariant,
    input::{InputEvent, InputState},
};
use std::cell::Cell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GalleryEvent {
    Open(PathBuf),
    Edit(PathBuf),
    OpenStudio,
    OpenUrl(String),
}

pub struct Gallery {
    pub(super) history: Entity<History>,
    pub(super) settings: Entity<SettingsModel>,
    pub(super) paths: Paths,
    pub(super) files: Arc<dyn GalleryFiles>,
    pub(super) shares: Option<Entity<Shares>>,
    pub(super) clipboard: Option<Rc<dyn ClipboardSink>>,
    pub(super) items: Vec<Item>,
    pub(super) cloud: Vec<Item>,
    pub(super) shown: Vec<Item>,
    pub(super) section: Section,
    pub(super) category: Category,
    /// Links whose delete has been sent and not yet answered.
    pub(super) deleting: HashSet<String>,
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
    pub(super) size_focus: FocusHandle,
    pub(super) size_track: Rc<Cell<Bounds<Pixels>>>,
    pub(super) size_dragging: bool,
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
        settings: Entity<SettingsModel>,
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
        let on_settings = cx.observe(&settings, |_, _, cx| cx.notify());
        let mut gallery = Self {
            history,
            settings,
            paths,
            files,
            shares: None,
            clipboard: None,
            items: Vec::new(),
            cloud: Vec::new(),
            shown: Vec::new(),
            section: Section::default(),
            category: Category::All,
            deleting: HashSet::new(),
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
            size_focus: cx.focus_handle().tab_stop(true),
            size_track: Rc::default(),
            size_dragging: false,
            _subscriptions: vec![on_search, on_history, on_settings],
        };
        gallery.sync(cx);
        gallery
    }

    /// Lists every link in `shares` and copies them through `clipboard`.
    pub fn with_shares(
        mut self,
        shares: Option<Entity<Shares>>,
        clipboard: Rc<dyn ClipboardSink>,
        cx: &mut Context<Self>,
    ) -> Self {
        if let Some(shares) = &shares {
            self._subscriptions
                .push(cx.subscribe(shares, |this, _, _: &SharesChanged, cx| this.sync(cx)));
            self._subscriptions
                .push(cx.subscribe(shares, |this, _, e: &ShareDeleted, cx| {
                    this.on_deleted(e, cx)
                }));
        }
        self.shares = shares;
        self.clipboard = Some(clipboard);
        self.sync(cx);
        self
    }

    pub fn shown(&self) -> &[Item] {
        &self.shown
    }

    pub fn count(&self, category: Category) -> usize {
        model::count(&self.items, category)
    }

    pub fn count_in(&self, section: Section, category: Category) -> usize {
        model::count(self.section_items(section), category)
    }

    fn section_items(&self, section: Section) -> &[Item] {
        match section {
            Section::Local => &self.items,
            Section::Cloud => &self.cloud,
        }
    }

    pub fn section(&self) -> Section {
        self.section
    }

    pub fn set_section(&mut self, section: Section, category: Category, cx: &mut Context<Self>) {
        self.section = section;
        self.category = category;
        self.refilter(cx);
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

    /// How far the icon grid is scrolled down, in pixels.
    pub fn grid_scrolled(&self) -> f32 {
        -self.grid_scroll.0.borrow().base_handle.offset().y.as_f32()
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

    /// Thumbnails cached sharp enough for the current item size.
    pub fn thumbnails_sharp(&self, cx: &App) -> usize {
        self.thumbs
            .ready_at(model::thumb_pixels(self.item_size(cx)))
    }

    /// The icon view's preview side in pixels, from the config.
    pub fn item_size(&self, cx: &App) -> u32 {
        model::item_size(self.settings.read(cx).config().gallery.item_size)
    }

    pub fn set_item_size(&mut self, size: u32, cx: &mut Context<Self>) {
        let size = model::item_size(size);
        if size == self.item_size(cx) {
            return;
        }
        self.settings
            .update(cx, |s, cx| s.update(|c| c.gallery.item_size = size, cx));
        cx.notify();
    }

    /// Drops entries whose file is gone and retries thumbnails that failed.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.thumbs = Thumbs::default();
        self.sync(cx);
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        let records = match &self.shares {
            Some(shares) => shares.read(cx).all().to_vec(),
            None => Vec::new(),
        };
        let entries = self.history.read(cx).entries();
        self.items = model::local(&self.paths, entries);
        shared::attach(&mut self.items, &records, OffsetDateTime::now_utc());
        self.cloud = shared::cloud(&records, &self.items, entries);
        self.deleting
            .retain(|id| records.iter().any(|r| &r.id == id));
        let (items, cloud) = (&self.items, &self.cloud);
        self.thumbs
            .retain(|file| items.iter().chain(cloud).any(|i| i.file.as_path() == file));
        self.refilter(cx);
    }

    fn refilter(&mut self, cx: &mut Context<Self>) {
        self.shown = model::visible(
            self.section_items(self.section),
            self.category,
            &self.query,
            self.sort,
        );
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
        self.items.iter().chain(&self.cloud).find(|i| i.id == id)
    }

    pub fn open(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(file) = self.live_file(id, cx) {
            cx.emit(GalleryEvent::Open(file));
        }
    }

    pub fn activate(&mut self, id: Uuid, cx: &mut Context<Self>) {
        match self.item(id).map(|i| i.kind) {
            Some(MediaKind::Screenshot) => self.edit(id, cx),
            Some(MediaKind::Video) => self.open(id, cx),
            None => {}
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

    pub fn open_studio(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if self.item(id).is_some_and(|i| i.kind == MediaKind::Video) {
            cx.emit(GalleryEvent::OpenStudio);
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
        let Some(item) = self.items.iter().find(|i| i.id == id).cloned() else {
            return Ok(());
        };
        let result = if item.kind == MediaKind::Video {
            self.files
                .trash(item.project.as_deref().unwrap_or(&item.file))
                .map(|()| self.history.update(cx, |h, cx| h.videos_changed(cx)))
        } else {
            capture_files(&item.file)
                .iter()
                .try_for_each(|file| self.files.trash(file))
                .and_then(|()| {
                    self.history
                        .update(cx, |history, cx| history.remove(id, cx))
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                })
        };
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
        let Some(item) = self.items.iter().find(|i| i.id == id) else {
            return;
        };
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
        let pixels = match self.view {
            ViewMode::Icons => model::thumb_pixels(self.item_size(cx)),
            ViewMode::List => model::thumb_pixels(model::SIZE_MIN),
        };
        let wanted: Vec<(PathBuf, MediaKind)> = self.shown
            [range.start.min(self.shown.len())..range.end.min(self.shown.len())]
            .iter()
            .map(|i| (i.file.clone(), i.kind))
            .collect();
        for (file, kind) in wanted {
            if !self.thumbs.claim(&file, pixels) {
                continue;
            }
            let (paths, files, source) = (self.paths.clone(), self.files.clone(), file.clone());
            let load = cx.background_spawn(async move {
                thumbs::load(&paths, files.as_ref(), kind, &source, pixels)
            });
            cx.spawn(async move |this, cx| {
                let image = load.await;
                let _ = this.update(cx, |this, cx| {
                    this.thumbs.finish(&file, pixels, image);
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
