#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _, vec, Address, Env, String, Vec,
};

fn setup() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, QuestContract);
    let creator = Address::generate(&env);
    (env, contract_id, creator)
}

fn s(env: &Env, v: &str) -> String {
    String::from_str(env, v)
}

#[test]
fn create_quest_stores_tags_on_chain() {
    let (env, contract_id, creator) = setup();
    let client = QuestContractClient::new(&env, &contract_id);

    let tags: Vec<String> = vec![&env, s(&env, "combat"), s(&env, "exploration")];

    let quest_id = client.create_quest(
        &creator,
        &s(&env, "Slay the dragon"),
        &s(&env, "Defeat the ancient dragon"),
        &tags,
        &1000_i128,
    );

    let quest = client.get_quest(&quest_id);
    assert_eq!(quest.id, quest_id);
    assert_eq!(quest.creator, creator);
    assert_eq!(quest.tags.len(), 2);
    assert_eq!(quest.tags.get(0).unwrap(), s(&env, "combat"));
    assert_eq!(quest.tags.get(1).unwrap(), s(&env, "exploration"));
    assert_eq!(quest.status, QuestStatus::Active);
}

#[test]
fn get_quests_by_tag_returns_only_matching_quests() {
    let (env, contract_id, creator) = setup();
    let client = QuestContractClient::new(&env, &contract_id);

    // Quest A: combat
    let a = client.create_quest(
        &creator,
        &s(&env, "Goblin Hunt"),
        &s(&env, "Hunt down 10 goblins"),
        &vec![&env, s(&env, "combat")],
        &100_i128,
    );

    // Quest B: crafting
    let b = client.create_quest(
        &creator,
        &s(&env, "Forge a Sword"),
        &s(&env, "Craft a steel sword"),
        &vec![&env, s(&env, "crafting")],
        &50_i128,
    );

    // Quest C: combat + exploration
    let c = client.create_quest(
        &creator,
        &s(&env, "Dungeon Dive"),
        &s(&env, "Clear a dungeon"),
        &vec![&env, s(&env, "combat"), s(&env, "exploration")],
        &200_i128,
    );

    let combat_quests = client.get_quests_by_tag(&s(&env, "combat"));
    assert_eq!(combat_quests.len(), 2);
    assert_eq!(combat_quests.get(0).unwrap().id, a);
    assert_eq!(combat_quests.get(1).unwrap().id, c);

    let crafting_quests = client.get_quests_by_tag(&s(&env, "crafting"));
    assert_eq!(crafting_quests.len(), 1);
    assert_eq!(crafting_quests.get(0).unwrap().id, b);

    let exploration_quests = client.get_quests_by_tag(&s(&env, "exploration"));
    assert_eq!(exploration_quests.len(), 1);
    assert_eq!(exploration_quests.get(0).unwrap().id, c);
}

#[test]
fn get_quests_by_tag_returns_empty_for_unknown_tag() {
    let (env, contract_id, creator) = setup();
    let client = QuestContractClient::new(&env, &contract_id);

    client.create_quest(
        &creator,
        &s(&env, "Quest"),
        &s(&env, "desc"),
        &vec![&env, s(&env, "combat")],
        &1_i128,
    );

    let result = client.get_quests_by_tag(&s(&env, "fishing"));
    assert_eq!(result.len(), 0);
}

#[test]
fn create_quest_with_no_tags_is_allowed() {
    let (env, contract_id, creator) = setup();
    let client = QuestContractClient::new(&env, &contract_id);

    let id = client.create_quest(
        &creator,
        &s(&env, "Untagged"),
        &s(&env, "no tags"),
        &Vec::<String>::new(&env),
        &0_i128,
    );

    let quest = client.get_quest(&id);
    assert_eq!(quest.tags.len(), 0);
}

#[test]
fn get_quest_tags_returns_assigned_tags() {
    let (env, contract_id, creator) = setup();
    let client = QuestContractClient::new(&env, &contract_id);

    let tags: Vec<String> = vec![
        &env,
        s(&env, "pvp"),
        s(&env, "ranked"),
        s(&env, "season1"),
    ];
    let id = client.create_quest(
        &creator,
        &s(&env, "Arena Match"),
        &s(&env, "Win a ranked match"),
        &tags,
        &500_i128,
    );

    let returned = client.get_quest_tags(&id);
    assert_eq!(returned, tags);
}

#[test]
fn get_quest_ids_by_tag_lists_ids_in_creation_order() {
    let (env, contract_id, creator) = setup();
    let client = QuestContractClient::new(&env, &contract_id);

    let id1 = client.create_quest(
        &creator,
        &s(&env, "Q1"),
        &s(&env, "d"),
        &vec![&env, s(&env, "social")],
        &1_i128,
    );
    let id2 = client.create_quest(
        &creator,
        &s(&env, "Q2"),
        &s(&env, "d"),
        &vec![&env, s(&env, "social")],
        &1_i128,
    );

    let ids = client.get_quest_ids_by_tag(&s(&env, "social"));
    assert_eq!(ids.len(), 2);
    assert_eq!(ids.get(0).unwrap(), id1);
    assert_eq!(ids.get(1).unwrap(), id2);
}

#[test]
#[should_panic(expected = "quest contract error")]
fn create_quest_rejects_empty_title() {
    let (env, contract_id, creator) = setup();
    let client = QuestContractClient::new(&env, &contract_id);

    client.create_quest(
        &creator,
        &s(&env, ""),
        &s(&env, "desc"),
        &vec![&env, s(&env, "combat")],
        &0_i128,
    );
}

#[test]
#[should_panic(expected = "quest contract error")]
fn create_quest_rejects_empty_tag() {
    let (env, contract_id, creator) = setup();
    let client = QuestContractClient::new(&env, &contract_id);

    client.create_quest(
        &creator,
        &s(&env, "Title"),
        &s(&env, "desc"),
        &vec![&env, s(&env, "")],
        &0_i128,
    );
}

#[test]
#[should_panic(expected = "quest contract error")]
fn create_quest_rejects_too_many_tags() {
    let (env, contract_id, creator) = setup();
    let client = QuestContractClient::new(&env, &contract_id);

    let mut tags: Vec<String> = Vec::new(&env);
    for i in 0..(MAX_TAGS_PER_QUEST + 1) {
        // Create unique tag strings.
        let _ = i;
        tags.push_back(s(&env, "tag"));
    }

    client.create_quest(
        &creator,
        &s(&env, "Title"),
        &s(&env, "desc"),
        &tags,
        &0_i128,
    );
}

#[test]
#[should_panic(expected = "quest contract error")]
fn get_quest_panics_for_unknown_id() {
    let (env, contract_id, _creator) = setup();
    let client = QuestContractClient::new(&env, &contract_id);
    client.get_quest(&999_u64);
}

#[test]
fn create_quest_emits_event() {
    let (env, contract_id, creator) = setup();
    let client = QuestContractClient::new(&env, &contract_id);

    let _ = client.create_quest(
        &creator,
        &s(&env, "Title"),
        &s(&env, "desc"),
        &vec![&env, s(&env, "combat")],
        &0_i128,
    );

    // The contract publishes a `quest_created` event on creation.
    let all = env.events().all();
    assert!(all.len() >= 1);
}
