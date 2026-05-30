save_quest(&env, &quest);

quest_created(
    &env,
    quest.id,
    creator.clone(),
);

quest.id