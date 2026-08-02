use crate::types::{Config, Event, EventTicket, EventTicketError};
use soroban_sdk::{contracttype, symbol_short, Address, Env, Vec};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Global event-ticket configuration (instance storage).
    Config,
    /// Per-event metadata, keyed by event id (persistent storage).
    Event(u64),
    /// Per-ticket metadata, keyed by token id (persistent storage).
    Ticket(u64),
    /// Per-holder ticket id list, keyed by holder address (persistent storage).
    HolderTickets(Address),
    /// Per-event attendance counter, keyed by event id (persistent storage).
    Attendance(u64),
}

pub struct Storage;

impl Storage {
    pub fn has_config(env: &Env) -> bool {
        env.storage().instance().has(&DataKey::Config)
    }

    pub fn set_config(env: &Env, config: &Config) {
        env.storage().instance().set(&DataKey::Config, config);
    }

    pub fn get_config(env: &Env) -> Result<Config, EventTicketError> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(EventTicketError::NotInitialized)
    }

    pub fn set_event(env: &Env, event_id: u64, event: &Event) {
        env.storage()
            .persistent()
            .set(&DataKey::Event(event_id), event);
    }

    pub fn get_event(env: &Env, event_id: u64) -> Result<Event, EventTicketError> {
        env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .ok_or(EventTicketError::EventNotFound)
    }

    pub fn set_ticket(env: &Env, token_id: u64, ticket: &EventTicket) {
        env.storage()
            .persistent()
            .set(&DataKey::Ticket(token_id), ticket);
    }

    pub fn get_ticket(env: &Env, token_id: u64) -> Result<EventTicket, EventTicketError> {
        env.storage()
            .persistent()
            .get(&DataKey::Ticket(token_id))
            .ok_or(EventTicketError::TicketNotFound)
    }

    pub fn get_tickets_by_holder(env: &Env, holder: &Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::HolderTickets(holder.clone()))
            .unwrap_or_else(|| Vec::new(env))
    }

    pub fn add_ticket_to_holder(env: &Env, holder: &Address, token_id: u64) {
        let key = DataKey::HolderTickets(holder.clone());
        let mut tickets: Vec<u64> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));
        tickets.push_back(token_id);
        env.storage().persistent().set(&key, &tickets);
    }

    pub fn remove_ticket_from_holder(env: &Env, holder: &Address, token_id: u64) {
        let key = DataKey::HolderTickets(holder.clone());
        let tickets: Vec<u64> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));

        let mut new_tickets = Vec::new(env);
        for id in tickets.iter() {
            if id != token_id {
                new_tickets.push_back(id);
            }
        }

        env.storage().persistent().set(&key, &new_tickets);
    }

    pub fn get_event_attendance(env: &Env, event_id: u64) -> (u64, u64) {
        let attended: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Attendance(event_id))
            .unwrap_or(0);

        let event = Self::get_event(env, event_id).unwrap_or_else(|_| Event {
            id: event_id,
            name: symbol_short!(""),
            start_at: 0,
            end_at: 0,
            max_tickets: 0,
            tickets_issued: 0,
            status: crate::types::EventStatus::Upcoming,
        });

        (attended, event.tickets_issued)
    }

    pub fn increment_attendance(env: &Env, event_id: u64) {
        let attended: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Attendance(event_id))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Attendance(event_id), &(attended + 1));
    }
}
