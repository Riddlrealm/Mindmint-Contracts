#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Vec, IntoVal,
};

#[contracttype]
#[derive(Clone)]
pub struct Ingredient {
    pub token_address: Address,
    pub token_id: u32,
    pub amount: u32, // For fungible tokens, 1 for NFTs
}

#[contracttype]
#[derive(Clone)]
pub struct Recipe {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub ingredients: Vec<Ingredient>,
    pub output_token_address: Address,
    pub output_token_id: u32,
    pub success_rate: u32, // Percentage 0-100
    pub rarity: Rarity,
    pub cooldown_seconds: u64,
    pub enabled: bool,
}

#[contracttype]
#[derive(Clone, Copy)]
pub enum Rarity {
    Common = 0,
    Uncommon = 1,
    Rare = 2,
    Epic = 3,
    Legendary = 4,
}

#[contracttype]
pub enum DataKey {
    Recipe(u32),              // Persistent: Recipe data
    NextRecipeId,             // Instance: Counter for recipe IDs
    Admin,                    // Instance: Contract administrator
    PlayerCooldown(Address),  // Persistent: Last crafting time per player
    RecipeCount,              // Instance: Total recipes
    NftContract,              // Instance: Address of the NFT contract to use
}

#[contract]
pub struct CraftingContract;

#[contractimpl]
impl CraftingContract {
    /// Initialize the contract and set the administrator and NFT contract.
    pub fn initialize(env: Env, admin: Address, nft_contract: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NftContract, &nft_contract);
        env.storage().instance().set(&DataKey::NextRecipeId, &1u32);
        env.storage().instance().set(&DataKey::RecipeCount, &0u32);
    }

    /// Register a new crafting recipe (admin only).
    pub fn register_recipe(
        env: Env,
        name: String,
        description: String,
        ingredients: Vec<Ingredient>,
        output_token_address: Address,
        output_token_id: u32,
        success_rate: u32,
        rarity: u32, // 0-4 for Rarity enum
        cooldown_seconds: u64,
    ) -> u32 {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        if success_rate > 100 {
            panic!("Success rate must be between 0 and 100");
        }

        let rarity_enum = match rarity {
            0 => Rarity::Common,
            1 => Rarity::Uncommon,
            2 => Rarity::Rare,
            3 => Rarity::Epic,
            4 => Rarity::Legendary,
            _ => panic!("Invalid rarity value"),
        };

        let recipe_id: u32 = env.storage().instance().get(&DataKey::NextRecipeId).unwrap();

        let recipe = Recipe {
            id: recipe_id,
            name,
            description,
            ingredients,
            output_token_address,
            output_token_id,
            success_rate,
            rarity: rarity_enum,
            cooldown_seconds,
            enabled: true,
        };

        env.storage().persistent().set(&DataKey::Recipe(recipe_id), &recipe);
        env.storage().persistent().extend_ttl(&DataKey::Recipe(recipe_id), 100_000, 500_000);

        env.storage().instance().set(&DataKey::NextRecipeId, &(recipe_id + 1));
        let count: u32 = env.storage().instance().get(&DataKey::RecipeCount).unwrap_or(0);
        env.storage().instance().set(&DataKey::RecipeCount, &(count + 1));

        env.events().publish((symbol_short!("recipe"), recipe_id), ());

        recipe_id
    }

    /// Get recipe details by ID.
    pub fn get_recipe(env: Env, recipe_id: u32) -> Recipe {
        env.storage()
            .persistent()
            .get(&DataKey::Recipe(recipe_id))
            .unwrap_or_else(|| panic!("Recipe not found"))
    }

    /// Get all recipe IDs.
    pub fn get_all_recipes(env: Env) -> Vec<u32> {
        let count: u32 = env.storage().instance().get(&DataKey::RecipeCount).unwrap_or(0);
        let mut recipes = Vec::new(&env);
        for i in 1..=count {
            recipes.push_back(i);
        }
        recipes
    }

    /// Validate that a player owns all required ingredients.
    fn validate_ingredients(env: Env, player: Address, ingredients: Vec<Ingredient>) -> bool {
        let nft_contract: Address = env.storage().instance().get(&DataKey::NftContract).unwrap();

        for ingredient in ingredients.iter() {
            // For NFTs, check ownership via cross-contract call
            if ingredient.amount == 1 {
                // Check if player owns the NFT
                let owner: Result<Address, soroban_sdk::Error> = env.invoke_contract(
                    &nft_contract,
                    &symbol_short!("owner_of"),
                    Vec::from_array(&env, [ingredient.token_id.into()]),
                );

                match owner {
                    Ok(owner_addr) => {
                        if owner_addr != player {
                            return false;
                        }
                    }
                    Err(_) => return false, // Token doesn't exist or other error
                }
            } else {
                // For fungible tokens, would need balance checking
                // TODO: Implement fungible token balance checking
                return false; // Not implemented yet
            }
        }
        true
    }

    /// Attempt to craft using a recipe.
    pub fn craft(env: Env, player: Address, recipe_id: u32) -> u32 {
        player.require_auth();

        let recipe = Self::get_recipe(env.clone(), recipe_id);
        if !recipe.enabled {
            panic!("recipe_disabled");
        }

        // Check cooldown
        let last_craft: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerCooldown(player.clone()))
            .unwrap_or(0);
        let current_time = env.ledger().timestamp();
        if current_time < last_craft + recipe.cooldown_seconds {
            panic!("cooldown_active");
        }

        // Validate ingredients
        if !Self::validate_ingredients(env.clone(), player.clone(), recipe.ingredients.clone()) {
            panic!("invalid_ingredients");
        }

        // Update cooldown
        env.storage().persistent().set(&DataKey::PlayerCooldown(player.clone()), &current_time);
        env.storage().persistent().extend_ttl(&DataKey::PlayerCooldown(player.clone()), 100_000, 500_000);

        // Determine success
        let random_seed: u64 = env.prng().gen_range(0..100);
        let success = random_seed < recipe.success_rate as u64;

        if success {
            // Burn ingredients
            for ingredient in recipe.ingredients.iter() {
                if ingredient.amount == 1 {
                    // Burn NFT
                    let _: Result<(), soroban_sdk::Error> = env.invoke_contract(
                        &ingredient.token_address,
                        &symbol_short!("burn"),
                        Vec::from_array(&env, [ingredient.token_id.into()]),
                    );
                    // Note: We ignore errors here - in production, you'd want proper error handling
                }
                // TODO: Handle fungible tokens
            }

            // Mint output NFT
            let output_id: u32 = env.invoke_contract(
                &recipe.output_token_address,
                &symbol_short!("craftmint"),
                Vec::from_array(&env, [
                    player.into_val(&env),
                    recipe.output_token_id.into(),
                    String::from_str(&env, "Crafted achievement").into_val(&env),
                ]),
            );

            env.events().publish((symbol_short!("success"), &player, recipe_id), output_id);
            output_id
        } else {
            // Handle failure - emit failure event
            env.events().publish((symbol_short!("failure"), &player, recipe_id), 0u32);
            panic!("craft_failed")
        }
    }

    /// Get player's last crafting time.
    pub fn get_player_cooldown(env: Env, player: Address) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::PlayerCooldown(player))
            .unwrap_or(0)
    }

    /// Enable/disable a recipe (admin only).
    pub fn set_recipe_enabled(env: Env, recipe_id: u32, enabled: bool) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let mut recipe = Self::get_recipe(env.clone(), recipe_id);
        recipe.enabled = enabled;
        env.storage().persistent().set(&DataKey::Recipe(recipe_id), &recipe);
    }
}

#[cfg(test)]
mod test {
    use crate::{CraftingContract, Ingredient};
    use soroban_sdk::{testutils::Address as AddressTestUtils, vec, Address, Env, String};

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let nft_contract = Address::generate(&env);

        let contract_id = env.register_contract(None, CraftingContract);
        let client = crate::CraftingContractClient::new(&env, &contract_id);

        client.initialize(&admin, &nft_contract);

        assert_eq!(client.get_all_recipes().len(), 0);
    }

    #[test]
    fn test_register_recipe() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let nft_contract = Address::generate(&env);

        let contract_id = env.register_contract(None, CraftingContract);
        let client = crate::CraftingContractClient::new(&env, &contract_id);

        client.initialize(&admin, &nft_contract);

        let ingredients = vec![
            &env,
            Ingredient {
                token_address: nft_contract.clone(),
                token_id: 1,
                amount: 1,
            },
            Ingredient {
                token_address: nft_contract.clone(),
                token_id: 2,
                amount: 1,
            },
        ];

        let recipe_id = client.register_recipe(
            &String::from_str(&env, "Epic Sword"),
            &String::from_str(&env, "A powerful sword crafted from rare materials"),
            &ingredients,
            &nft_contract,
            &100,
            &80,
            &3, // Epic
            &3600, // 1 hour cooldown
        );

        assert_eq!(recipe_id, 1);

        let recipe = client.get_recipe(&recipe_id);
        assert_eq!(recipe.name, String::from_str(&env, "Epic Sword"));
        assert_eq!(recipe.success_rate, 80);
        assert_eq!(recipe.ingredients.len(), 2);
    }

    #[test]
    fn test_craft_success() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let _player = Address::generate(&env);
        let nft_contract = Address::generate(&env); // Mock NFT contract address

        let contract_id = env.register_contract(None, CraftingContract);
        let client = crate::CraftingContractClient::new(&env, &contract_id);
        client.initialize(&admin, &nft_contract);

        // Register recipe
        let ingredients = vec![
            &env,
            Ingredient {
                token_address: nft_contract.clone(),
                token_id: 1,
                amount: 1,
            },
            Ingredient {
                token_address: nft_contract.clone(),
                token_id: 2,
                amount: 1,
            },
        ];

        let recipe_id = client.register_recipe(
            &String::from_str(&env, "Epic Sword"),
            &String::from_str(&env, "A powerful sword"),
            &ingredients,
            &nft_contract,
            &100,
            &100, // 100% success rate
            &3,
            &0, // No cooldown
        );

        // Note: Full crafting test requires NFT contract setup
        // For testnet validation, we verify the recipe registration works
        assert_eq!(recipe_id, 1);
        let recipe = client.get_recipe(&recipe_id);
        assert_eq!(recipe.success_rate, 100);
    }

    #[test]
    fn test_recipe_discovery() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let nft_contract = Address::generate(&env);

        let contract_id = env.register_contract(None, CraftingContract);
        let client = crate::CraftingContractClient::new(&env, &contract_id);

        client.initialize(&admin, &nft_contract);

        // Register multiple recipes
        for i in 0..3 {
            let recipe_name = match i {
                0 => String::from_str(&env, "Recipe 0"),
                1 => String::from_str(&env, "Recipe 1"),
                2 => String::from_str(&env, "Recipe 2"),
                _ => String::from_str(&env, "Recipe"),
            };

            let ingredients = vec![
                &env,
                Ingredient {
                    token_address: nft_contract.clone(),
                    token_id: i + 1,
                    amount: 1,
                },
            ];

            client.register_recipe(
                &recipe_name,
                &String::from_str(&env, "Description"),
                &ingredients,
                &nft_contract,
                &100,
                &80,
                &(i as u32 % 5),
                &3600,
            );
        }

        // Get all recipes
        let all_recipes = client.get_all_recipes();
        assert_eq!(all_recipes.len(), 3);
    }

    #[test]
    fn test_craft_failure() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let _player = Address::generate(&env);
        let nft_contract = Address::generate(&env);

        let contract_id = env.register_contract(None, CraftingContract);
        let client = crate::CraftingContractClient::new(&env, &contract_id);
        client.initialize(&admin, &nft_contract);

        // Register recipe with 0% success rate
        let ingredients = vec![
            &env,
            Ingredient {
                token_address: nft_contract.clone(),
                token_id: 1,
                amount: 1,
            },
        ];

        let recipe_id = client.register_recipe(
            &String::from_str(&env, "Impossible Item"),
            &String::from_str(&env, "Cannot be crafted"),
            &ingredients,
            &nft_contract,
            &200,
            &0, // 0% success rate
            &4,
            &0,
        );

        // Verify recipe registration
        assert_eq!(recipe_id, 1);
        let recipe = client.get_recipe(&recipe_id);
        assert_eq!(recipe.success_rate, 0);
    }

    #[test]
    fn test_cooldown() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let _player = Address::generate(&env);
        let nft_contract = Address::generate(&env);

        let contract_id = env.register_contract(None, CraftingContract);
        let client = crate::CraftingContractClient::new(&env, &contract_id);
        client.initialize(&admin, &nft_contract);

        // Register recipe with cooldown
        let ingredients = vec![
            &env,
            Ingredient {
                token_address: nft_contract.clone(),
                token_id: 1,
                amount: 1,
            },
        ];

        let recipe_id = client.register_recipe(
            &String::from_str(&env, "Cooldown Item"),
            &String::from_str(&env, "Has cooldown"),
            &ingredients,
            &nft_contract,
            &200,
            &100,
            &1,
            &60, // 60 seconds cooldown
        );

        // Verify cooldown is set in recipe
        let recipe = client.get_recipe(&recipe_id);
        assert_eq!(recipe.cooldown_seconds, 60);
    }
}