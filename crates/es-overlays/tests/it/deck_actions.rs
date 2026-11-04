#![allow(clippy::unwrap_used)]
use crate::deck::rig_with;
use es_app::deck::DeckAction;
use es_model::config::CardActions;
use es_overlays::deck::view::DeckSettings;

fn every_action() -> CardActions {
    CardActions {
        preview: true,
        delete: true,
        ..CardActions::default()
    }
}

#[test]
fn a_card_draws_only_the_actions_chosen_in_settings() {
    let settings = DeckSettings {
        actions: every_action(),
        ..DeckSettings::default()
    };
    let mut r = rig_with(1, false, settings, Some("shot1.png"));
    let c = r.card(0);
    r.click(c.center().x as f32, (c.y0 + 19.) as f32);
    assert_eq!(r.requested(), [("shot1".into(), DeckAction::Preview)]);
    assert_eq!(r.names(), ["shot1"], "a preview keeps the card");
    r.click(c.center().x as f32, (c.y1 - 19.) as f32);
    assert_eq!(
        r.requested().last(),
        Some(&("shot1".into(), DeckAction::Delete)),
        "an auto-saved card offers Delete too"
    );
    assert!(r.names().is_empty(), "Delete takes the card away");

    let mut r = rig_with(1, false, DeckSettings::default(), None);
    let c = r.card(0);
    r.click(c.center().x as f32, (c.y0 + 19.) as f32);
    assert_eq!(
        r.requested(),
        [("shot1".into(), DeckAction::Edit)],
        "with the default set the top middle is picture, so a click there edits"
    );

    let settings = DeckSettings {
        actions: CardActions {
            copy: false,
            ..CardActions::default()
        },
        ..DeckSettings::default()
    };
    let mut r = rig_with(1, false, settings, None);
    let c = r.card(0);
    r.click((c.center().x - 40.) as f32, c.center().y as f32);
    assert_eq!(
        r.requested(),
        [("shot1".into(), DeckAction::Edit)],
        "a card without Copy has no Copy button"
    );
}

#[test]
fn pictures_of_a_card_with_the_default_and_with_every_action() {
    for dark in [false, true] {
        let theme = if dark { "dark" } else { "light" };
        for (name, actions) in [("default", CardActions::default()), ("all", every_action())] {
            let settings = DeckSettings {
                actions,
                share: true,
                always_show_actions: true,
                ..DeckSettings::default()
            };
            let mut r = rig_with(1, dark, settings, None);
            r.next_frame();
            std::thread::sleep(std::time::Duration::from_millis(150));
            r.next_frame();
            r.s.app.screenshot(&format!("deck-card-{name}-{theme}"));
        }
    }
}
