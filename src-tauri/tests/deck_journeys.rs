// T1.2: deck import / restore / navigation / clear journeys through real IPC, real bus, real
// state machine, on a temp app-data dir (mock runtime only replaces the OS boundary).

mod common;

use std::path::{Path, PathBuf};

use common::TestApp;
use serde_json::{json, Value};
use streamdeck_lib::config::StreamDeckConfig;
use streamdeck_lib::events::AppEvent;
use streamdeck_lib::scene::SceneSource;
use tauri::Manager;

const TITLES: [&str; 5] = [
    "StreamDeck Test Deck",
    "Feature Overview",
    "Sample Code",
    "Visual Slide",
    "Unsupported Layout Demo",
];

fn fixture_pptx() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/streamdeck-test-deck.pptx")
        .to_str()
        .expect("utf-8 fixture path")
        .to_string()
}

fn active_id_file(t: &TestApp) -> PathBuf {
    t.data_dir.path().join("decks").join("active_deck_id.txt")
}

fn tags(events: &[AppEvent]) -> Vec<String> {
    events
        .iter()
        .map(|e| match e {
            AppEvent::SlideUpdate(p) => format!("SlideUpdate({}/{})", p.index, p.total),
            AppEvent::SceneSourceChanged { source } => format!("SceneSourceChanged({source:?})"),
            AppEvent::DeckImportProgress { slide, total } => format!("Progress({slide}/{total})"),
            AppEvent::DeckImportComplete { .. } => "ImportComplete".into(),
            other => format!("{other:?}"),
        })
        .collect()
}

fn import(t: &TestApp) -> String {
    t.invoke("deck_import", json!({ "path": fixture_pptx() }))
        .expect("fixture imports")
}

fn invoke_unit(t: &TestApp, cmd: &str, args: Value) {
    let _: () = t
        .invoke(cmd, args)
        .unwrap_or_else(|e| panic!("{cmd} failed: {e}"));
}

fn current(t: &TestApp) -> Value {
    t.invoke("slide_get_current", json!({}))
        .expect("slide_get_current")
}

#[tokio::test(flavor = "multi_thread")]
async fn import_publishes_progress_then_first_slide_then_scene_and_complete() {
    let t = TestApp::new();
    let mut events = t.collect_events();

    let deck_id = import(&t);
    let seen = events.drain();

    assert_eq!(
        tags(&seen),
        [
            "Progress(1/5)",
            "Progress(2/5)",
            "Progress(3/5)",
            "Progress(4/5)",
            "Progress(5/5)",
            "SlideUpdate(0/5)",
            "SceneSourceChanged(SlideDeck)",
            "ImportComplete",
        ]
    );
    let AppEvent::SlideUpdate(first) = &seen[5] else {
        panic!("expected SlideUpdate, got {:?}", seen[5]);
    };
    assert_eq!(first.title, TITLES[0]);
    assert_eq!(first.body, "A sample presentation for testing deck import.");
    assert!(matches!(
        &seen[7],
        AppEvent::DeckImportComplete { deck_id: id } if *id == deck_id
    ));

    let active = std::fs::read_to_string(active_id_file(&t)).expect("active id persisted");
    assert_eq!(active, deck_id);
    assert!(t
        .data_dir
        .path()
        .join("decks")
        .join(&deck_id)
        .join("deck.json")
        .is_file());
    assert_eq!(t.machine.read().await.total(), 5);

    let src: SceneSource = t
        .invoke("scene_get_active_source", json!({}))
        .expect("active source");
    assert_eq!(src, SceneSource::SlideDeck);

    let all: Vec<Value> = t
        .invoke("deck_get_all_slides", json!({}))
        .expect("all slides");
    let titles: Vec<&str> = all
        .iter()
        .map(|s| s["title"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(titles, TITLES);
}

#[tokio::test(flavor = "multi_thread")]
async fn import_switches_scene_back_to_slide_deck() {
    let t = TestApp::new();
    invoke_unit(&t, "scene_set_source", json!({ "source": "SceneMedia" }));

    import(&t);

    let src: SceneSource = t
        .invoke("scene_get_active_source", json!({}))
        .expect("active source");
    assert_eq!(src, SceneSource::SlideDeck);
}

#[tokio::test(flavor = "multi_thread")]
async fn import_rejects_bad_input_without_touching_state() {
    let t = TestApp::new();
    let mut events = t.collect_events();

    let unsupported: Result<String, Value> =
        t.invoke("deck_import", json!({ "path": "C:/nope/deck.txt" }));
    let err = unsupported.expect_err("unsupported extension is rejected");
    assert_eq!(err["type"], "InvalidInput");

    let missing: Result<String, Value> = t.invoke(
        "deck_import",
        json!({ "path": "C:/definitely/not/here/deck.pptx" }),
    );
    let err = missing.expect_err("missing file is rejected");
    assert_eq!(err["type"], "Internal");

    assert!(events.drain().is_empty(), "failed imports publish nothing");
    assert!(!active_id_file(&t).exists());
    assert_eq!(current(&t)["title"], "Welcome", "stub deck untouched");
}

#[tokio::test(flavor = "multi_thread")]
async fn navigation_publishes_advance_rewind_and_jump_events_and_persists_index() {
    let t = TestApp::new();
    import(&t);
    let mut events = t.collect_events();

    invoke_unit(&t, "slide_advance", json!({}));
    assert_eq!(tags(&events.drain()), ["SlideAdvance", "SlideUpdate(1/5)"]);
    assert_eq!(current(&t)["title"], TITLES[1]);

    invoke_unit(&t, "slide_jump_to", json!({ "index": 3 }));
    let seen = events.drain();
    assert_eq!(
        tags(&seen),
        ["SlideUpdate(3/5)"],
        "jump publishes no Advance/Rewind"
    );
    assert!(matches!(&seen[0], AppEvent::SlideUpdate(p) if p.title == TITLES[3]));

    invoke_unit(&t, "slide_rewind", json!({}));
    assert_eq!(tags(&events.drain()), ["SlideRewind", "SlideUpdate(2/5)"]);

    let persisted: Value = serde_json::from_slice(
        &std::fs::read(t.data_dir.path().join("slide_state.json")).expect("state persisted"),
    )
    .expect("state is json");
    assert_eq!(persisted["current_index"], 2);
    assert!(
        persisted["timer_started_at"].is_u64(),
        "advancing starts the timer"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn advance_past_last_slide_emits_deck_end_instead_of_slide_update() {
    let t = TestApp::new();
    import(&t);
    invoke_unit(&t, "slide_jump_to", json!({ "index": 4 }));
    let mut events = t.collect_events();

    invoke_unit(&t, "slide_advance", json!({}));

    assert_eq!(tags(&events.drain()), ["DeckEnd"]);
    assert_eq!(current(&t)["index"], 4, "index stays on the last slide");
}

#[tokio::test(flavor = "multi_thread")]
async fn rewind_before_first_slide_emits_deck_start_instead_of_slide_update() {
    let t = TestApp::new();
    import(&t);
    let mut events = t.collect_events();

    invoke_unit(&t, "slide_rewind", json!({}));

    assert_eq!(tags(&events.drain()), ["DeckStart"]);
    assert_eq!(current(&t)["index"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn jump_out_of_range_is_rejected_without_events_or_index_change() {
    let t = TestApp::new();
    import(&t);
    invoke_unit(&t, "slide_jump_to", json!({ "index": 2 }));
    let mut events = t.collect_events();

    let res: Result<(), Value> = t.invoke("slide_jump_to", json!({ "index": 5 }));

    let err = res.expect_err("index == total is out of range");
    assert_eq!(err["type"], "InvalidInput");
    assert!(events.drain().is_empty());
    assert_eq!(current(&t)["index"], 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn clear_publishes_empty_payload_and_scene_source_and_removes_active_id() {
    let t = TestApp::new();
    import(&t);
    assert!(active_id_file(&t).exists());
    let mut events = t.collect_events();

    invoke_unit(&t, "deck_clear", json!({}));

    let seen = events.drain();
    assert_eq!(
        tags(&seen),
        ["SlideUpdate(0/0)", "SceneSourceChanged(SlideDeck)"]
    );
    assert!(matches!(
        &seen[0],
        AppEvent::SlideUpdate(p) if p.title.is_empty() && p.body.is_empty()
    ));
    assert!(!active_id_file(&t).exists());

    let all: Vec<Value> = t
        .invoke("deck_get_all_slides", json!({}))
        .expect("all slides");
    assert!(all.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn clear_on_empty_deck_is_idempotent_and_boundaries_hold_on_empty_deck() {
    let t = TestApp::new();
    invoke_unit(&t, "deck_clear", json!({}));
    let mut events = t.collect_events();

    invoke_unit(&t, "deck_clear", json!({}));
    assert_eq!(
        tags(&events.drain()),
        ["SlideUpdate(0/0)", "SceneSourceChanged(SlideDeck)"]
    );

    invoke_unit(&t, "slide_advance", json!({}));
    assert_eq!(tags(&events.drain()), ["DeckEnd"]);
    invoke_unit(&t, "slide_rewind", json!({}));
    assert_eq!(tags(&events.drain()), ["DeckStart"]);

    let res: Result<(), Value> = t.invoke("slide_jump_to", json!({ "index": 0 }));
    assert_eq!(
        res.expect_err("no slide 0 in an empty deck")["type"],
        "InvalidInput"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_after_restart_reloads_the_imported_deck() {
    let first = TestApp::new();
    let deck_id = import(&first);
    let TestApp { data_dir, .. } = first;

    let t = TestApp::with_data_dir(data_dir, StreamDeckConfig::default());
    assert_eq!(current(&t)["title"], "Welcome", "fresh app starts on stub");
    let mut events = t.collect_events();

    invoke_unit(&t, "deck_restore", json!({}));

    let seen = events.drain();
    assert_eq!(tags(&seen), ["SlideUpdate(0/5)"]);
    assert!(matches!(&seen[0], AppEvent::SlideUpdate(p) if p.title == TITLES[0]));
    assert_eq!(
        std::fs::read_to_string(active_id_file(&t)).unwrap(),
        deck_id
    );
    assert_eq!(t.machine.read().await.total(), 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_is_a_noop_without_a_persisted_or_loadable_deck() {
    let t = TestApp::new();
    let mut events = t.collect_events();

    invoke_unit(&t, "deck_restore", json!({}));
    assert!(events.drain().is_empty(), "nothing persisted");

    let decks = t.data_dir.path().join("decks");
    std::fs::create_dir_all(&decks).unwrap();
    std::fs::write(decks.join("active_deck_id.txt"), "ghost").unwrap();
    invoke_unit(&t, "deck_restore", json!({}));
    assert!(events.drain().is_empty(), "dangling id without deck.json");
    assert_eq!(current(&t)["title"], "Welcome");
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_reports_corrupt_deck_json_and_keeps_current_deck() {
    let t = TestApp::new();
    let deck_dir = t.data_dir.path().join("decks").join("bad");
    std::fs::create_dir_all(&deck_dir).unwrap();
    std::fs::write(deck_dir.join("deck.json"), "{ not json").unwrap();
    std::fs::write(
        t.data_dir.path().join("decks").join("active_deck_id.txt"),
        "bad",
    )
    .unwrap();
    let mut events = t.collect_events();

    let res: Result<(), Value> = t.invoke("deck_restore", json!({}));

    assert_eq!(res.expect_err("corrupt deck.json")["type"], "Internal");
    assert!(events.drain().is_empty());
    assert_eq!(current(&t)["title"], "Welcome");
}

#[tokio::test(flavor = "multi_thread")]
async fn clear_then_restart_does_not_resurrect_the_deck() {
    let first = TestApp::new();
    import(&first);
    invoke_unit(&first, "deck_clear", json!({}));
    let TestApp { data_dir, .. } = first;

    let t = TestApp::with_data_dir(data_dir, StreamDeckConfig::default());
    let mut events = t.collect_events();
    invoke_unit(&t, "deck_restore", json!({}));

    assert!(events.drain().is_empty());
    assert_eq!(current(&t)["title"], "Welcome");
    assert_eq!(
        t.machine.read().await.total(),
        5,
        "stub deck, not the cleared one"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn importing_again_over_a_navigated_deck_resets_index_and_timer() {
    let t = TestApp::new();
    import(&t);
    invoke_unit(&t, "slide_jump_to", json!({ "index": 3 }));
    invoke_unit(&t, "slide_advance", json!({}));
    assert_eq!(current(&t)["index"], 4);
    assert!(t.machine.read().await.timer_started_at().is_some());

    import(&t);

    assert_eq!(current(&t)["index"], 0);
    assert_eq!(current(&t)["title"], TITLES[0]);
    assert_eq!(t.machine.read().await.timer_started_at(), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn startup_restore_activates_the_persisted_deck_with_saved_position() {
    let first = TestApp::new();
    import(&first);
    invoke_unit(&first, "slide_advance", json!({}));
    let started = first.machine.read().await.timer_started_at();
    assert!(started.is_some());
    let TestApp { data_dir, .. } = first;
    let t = TestApp::with_data_dir(data_dir, StreamDeckConfig::default());
    let mut events = t.collect_events();

    streamdeck_lib::state::restore_on_startup(&t.machine, &t.bus, t.data_dir.path()).await;

    let seen = events.drain();
    let Some(AppEvent::SlideUpdate(last)) = seen.last() else {
        panic!("expected a final SlideUpdate, got {seen:?}");
    };
    assert_eq!((last.index, last.total), (1, 5));
    assert_eq!(last.title, TITLES[1]);
    assert_eq!(current(&t)["title"], TITLES[1]);
    assert_eq!(t.machine.read().await.timer_started_at(), started);
}

#[tokio::test(flavor = "multi_thread")]
async fn startup_restore_keeps_the_saved_slide_index_of_the_persisted_deck() {
    let first = TestApp::new();
    import(&first);
    invoke_unit(&first, "slide_jump_to", json!({ "index": 3 }));
    invoke_unit(&first, "slide_advance", json!({}));
    let started = first.machine.read().await.timer_started_at();
    assert!(started.is_some());
    let TestApp { data_dir, .. } = first;
    let t = TestApp::with_data_dir(data_dir, StreamDeckConfig::default());

    streamdeck_lib::state::restore_on_startup(&t.machine, &t.bus, t.data_dir.path()).await;

    assert_eq!(t.machine.read().await.current_index(), 4);
    assert_eq!(t.machine.read().await.timer_started_at(), started);
}

#[tokio::test(flavor = "multi_thread")]
async fn spawn_timer_manages_abort_handle_and_publishes_ticks() {
    let t = TestApp::new();
    invoke_unit(&t, "slide_advance", json!({}));
    let mut events = t.collect_events();

    streamdeck_lib::state::spawn_timer(&t.app, t.machine.clone(), t.bus.clone());

    assert!(t
        .app
        .try_state::<streamdeck_lib::state::TimerAbortHandle>()
        .is_some());
    let is_tick = |e: &AppEvent| matches!(e, AppEvent::TimerTick { .. });
    let seen = events
        .collect_until(std::time::Duration::from_secs(5), |evs| {
            evs.iter().any(is_tick)
        })
        .await;
    assert!(
        seen.iter().any(is_tick),
        "no TimerTick within deadline: {seen:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn corrupt_deck_json_recovers_from_bak_and_reports_a_notice() {
    let first = TestApp::new();
    import(&first);
    let TestApp { data_dir, .. } = first;
    let id = std::fs::read_to_string(data_dir.path().join("decks/active_deck_id.txt")).expect("id");
    let deck_json = data_dir
        .path()
        .join("decks")
        .join(id.trim())
        .join("deck.json");
    let good = std::fs::read(&deck_json).expect("deck.json");
    std::fs::write(streamdeck_lib::persist::backup_path(&deck_json), &good).expect("bak");
    std::fs::write(&deck_json, b"{\"slides\": [ trunc").expect("corrupt");

    let t = TestApp::with_data_dir(data_dir, StreamDeckConfig::default());
    let notices =
        streamdeck_lib::state::restore_on_startup(&t.machine, &t.bus, t.data_dir.path()).await;

    assert_eq!(t.machine.read().await.total(), 5, "deck restored from .bak");
    assert_eq!(notices.len(), 1);
    assert!(
        notices[0].contains("previous saved copy was restored"),
        "{notices:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn corrupt_deck_json_without_bak_does_not_abort_startup() {
    let first = TestApp::new();
    import(&first);
    let TestApp { data_dir, .. } = first;
    let id = std::fs::read_to_string(data_dir.path().join("decks/active_deck_id.txt")).expect("id");
    let deck_json = data_dir
        .path()
        .join("decks")
        .join(id.trim())
        .join("deck.json");
    std::fs::write(&deck_json, b"not json").expect("corrupt");
    let _ = std::fs::remove_file(streamdeck_lib::persist::backup_path(&deck_json));

    let t = TestApp::with_data_dir(data_dir, StreamDeckConfig::default());
    let notices =
        streamdeck_lib::state::restore_on_startup(&t.machine, &t.bus, t.data_dir.path()).await;

    assert_eq!(notices.len(), 1);
    assert!(notices[0].contains("defaults are in use"), "{notices:?}");
}
