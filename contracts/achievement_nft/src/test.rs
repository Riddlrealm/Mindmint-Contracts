#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_nft_lifecycle() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, AchievementNFT);
        let client = AchievementNFTClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let user_a = Address::generate(&env);
        let user_b = Address::generate(&env);

        client.initialize(&admin);

        // 1. Test Mint
        let metadata = String::from_str(&env, "Master Puzzler");
        let token_id = client.mint(&user_a, &42, &metadata);
        
        assert_eq!(token_id, 1);
        assert_eq!(client.owner_of(&token_id), user_a);
        assert_eq!(client.total_supply(), 1);

        // 2. Test Transfer
        client.transfer(&user_a, &user_b, &token_id);
        assert_eq!(client.owner_of(&token_id), user_b);

        // 3. Test Burn
        client.burn(&token_id);
        assert_eq!(client.total_supply(), 0);
    }
}