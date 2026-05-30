#[ink::contract]
mod yield_aggregator {
    use super::strategy::*;
    use super::position::*;
    use super::events::*;

    #[ink(storage)]
    pub struct YieldAggregator {
        strategies: ink::storage::Mapping<u32, Strategy>,
        positions: ink::storage::Mapping<ink::primitives::AccountId, AggregatorPosition>,
    }

    impl YieldAggregator {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {
                strategies: Default::default(),
                positions: Default::default(),
            }
        }

        #[ink(message)]
        pub fn deposit(&mut self, amount: u128) {
            let caller = self.env().caller();
            let best = self.get_best_strategy();
            // simplified: record position
            let pos = AggregatorPosition {
                depositor: caller,
                total_deposited: amount,
                current_strategy_id: best.id,
                last_rebalance: self.env().block_timestamp(),
                total_earned: 0,
            };
            self.positions.insert(caller, &pos);
            self.env().emit_event(Deposited { depositor: caller, amount, strategy_id: best.id });
        }

        #[ink(message)]
        pub fn withdraw(&mut self, amount: u128) {
            let caller = self.env().caller();
            let mut pos = self.positions.get(caller).unwrap();
            assert!(pos.total_deposited >= amount, "Insufficient balance");
            pos.total_deposited -= amount;
            self.positions.insert(caller, &pos);
            self.env().emit_event(Withdrawn { depositor: caller, amount, strategy_id: pos.current_strategy_id });
        }

        #[ink(message)]
        pub fn rebalance(&mut self, depositor: ink::primitives::AccountId) {
            let mut pos = self.positions.get(depositor).unwrap();
            let best = self.get_best_strategy();
            let current = self.strategies.get(pos.current_strategy_id).unwrap();
            if best.current_apy_bps > current.current_apy_bps + 50 {
                pos.current_strategy_id = best.id;
                pos.last_rebalance = self.env().block_timestamp();
                self.positions.insert(depositor, &pos);
                self.env().emit_event(Rebalanced { depositor, from_strategy: current.id, to_strategy: best.id });
            }
        }

        #[ink(message)]
        pub fn register_strategy(&mut self, id: u32, addr: ink::primitives::AccountId, stype: StrategyType) {
            let strat = Strategy { id, contract_address: addr, strategy_type: stype, current_apy_bps: 0, total_deposited: 0 };
            self.strategies.insert(id, &strat);
        }

        #[ink(message)]
        pub fn update_strategy_apy(&mut self, id: u32, new_apy: u32) {
            let mut strat = self.strategies.get(id).unwrap();
            strat.current_apy_bps = new_apy;
            self.strategies.insert(id, &strat);
        }

        fn get_best_strategy(&self) -> Strategy {
            // simplified: iterate strategies and return max APY
            let mut best: Option<Strategy> = None;
            for id in 0..100 {
                if let Some(s) = self.strategies.get(id) {
                    if best.is_none() || s.current_apy_bps > best.as_ref().unwrap().current_apy_bps {
                        best = Some(s);
                    }
                }
            }
            best.expect("No strategies registered")
        }
    }
}
