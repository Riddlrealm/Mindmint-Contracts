#[test]
fn create_quest_emits_event() {
    let env = Env::default();

    let creator = Address::generate(&env);

    QuestContract::create_quest(
        env.clone(),
        creator.clone(),
        ...
    );

    let events = env.events().all();

    assert_eq!(events.len(), 1);
}

#[test]
fn update_quest_emits_event() {
    create quest

    update quest

    assert update event exists
}
#[test]
fn cancel_quest_emits_event() {
    create quest

    cancel quest

    assert cancelled event exists
}