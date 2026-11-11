use super::model::MediaKind;
use super::shared::ShareAction;
use super::view::Gallery;
use crate::strings::fl;
use es_ui::icons::IconName;
use gpui::{Context, WeakEntity, Window, prelude::FluentBuilder};
use gpui_component::{
    Icon,
    menu::{PopupMenu, PopupMenuItem},
};
use uuid::Uuid;

fn share_entry(action: ShareAction) -> (String, IconName) {
    match action {
        ShareAction::CopyLink => (fl!("gallery-menu-copy-link"), IconName::Link),
        ShareAction::OpenLink => (fl!("gallery-menu-open-link"), IconName::Cloud),
        ShareAction::CopyFileLink => (fl!("gallery-menu-copy-file-link"), IconName::Copy),
        ShareAction::Delete => (fl!("gallery-menu-delete-cloud"), IconName::Trash),
        ShareAction::Remove => (fl!("gallery-menu-remove-expired"), IconName::X),
    }
}

/// Read as the menu opens, so it offers what the item allows now.
pub(super) fn item_menu(
    menu: PopupMenu,
    view: &WeakEntity<Gallery>,
    id: Uuid,
    cx: &Context<PopupMenu>,
) -> PopupMenu {
    let Some(offer) = view.upgrade().and_then(|v| v.read(cx).offer(id)) else {
        return menu;
    };
    let act = |label: String,
               icon: IconName,
               f: fn(&mut Gallery, Uuid, &mut Window, &mut Context<Gallery>)| {
        let view = view.clone();
        PopupMenuItem::new(label)
            .icon(Icon::new(icon))
            .on_click(move |_, window, cx| {
                let _ = view.update(cx, |this, cx| f(this, id, window, cx));
            })
    };
    let share =
        |menu: PopupMenu, action: &ShareAction| {
            let (label, icon) = share_entry(*action);
            let (view, action) = (view.clone(), *action);
            menu.item(PopupMenuItem::new(label).icon(Icon::new(icon)).on_click(
                move |_, window, cx| {
                    let _ = view.update(cx, |this, cx| this.share_action(id, action, window, cx));
                },
            ))
        };
    let (kind, local) = (offer.kind, offer.local);
    let (links, ending): (Vec<ShareAction>, Vec<ShareAction>) = offer
        .share
        .iter()
        .partition(|a| !matches!(a, ShareAction::Delete | ShareAction::Remove));
    let menu = menu
        .item(
            act(
                fl!("gallery-menu-open"),
                IconName::SquareArrowOutUpRight,
                |g, id, _, cx| g.open(id, cx),
            )
            .disabled(!local),
        )
        .item(
            act(
                fl!("gallery-menu-edit"),
                IconName::Pencil,
                |g, id, _, cx| g.edit(id, cx),
            )
            .disabled(!local || kind != MediaKind::Screenshot),
        )
        .when(kind == MediaKind::Video, |menu| {
            menu.item(
                act(
                    fl!("gallery-menu-open-studio"),
                    IconName::Film,
                    |g, id, _, cx| g.open_studio(id, cx),
                )
                .disabled(!local),
            )
        })
        .item(
            act(
                fl!("gallery-menu-reveal"),
                IconName::FolderOpen,
                |g, id, _, cx| g.show_in_folder(id, cx),
            )
            .disabled(!local),
        )
        .when(!links.is_empty(), |menu| menu.separator());
    let menu = links.iter().fold(menu, share);
    let menu = menu.when(!ending.is_empty() || !offer.cloud, |menu| menu.separator());
    ending.iter().fold(menu, share).when(!offer.cloud, |menu| {
        menu.item(act(
            fl!("gallery-menu-trash"),
            IconName::Trash,
            |g, id, window, cx| g.confirm_trash(id, window, cx),
        ))
    })
}
