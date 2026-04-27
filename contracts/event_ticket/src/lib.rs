#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol, Vec};

mod storage;
mod types;

use storage::Storage;
use types::{Config, Event, EventStatus, EventTicket, EventTicketError, TicketTier};

#[contract]
pub struct EventTicketContract;

#[contractimpl]
impl EventTicketContract {
    pub fn initialize(env: Env, admin: Address, oracle: Address) -> Result<(), EventTicketError> {
        if Storage::has_config(&env) {
            return Err(EventTicketError::AlreadyInitialized);
        }

        let config = Config {
            admin,
            oracle,
            next_event_id: 1,
            next_token_id: 1,
        };
        Storage::set_config(&env, &config);

        Ok(())
    }

    pub fn create_event(
        env: Env,
        name: Symbol,
        start_at: u64,
        end_at: u64,
        max_tickets: u64,
    ) -> Result<u64, EventTicketError> {
        let mut config = Storage::get_config(&env)?;
        config.admin.require_auth();

        if start_at >= end_at {
            return Err(EventTicketError::EventNotFound);
        }

        let event_id = config.next_event_id;
        config.next_event_id += 1;

        let event = Event {
            id: event_id,
            name: name.clone(),
            start_at,
            end_at,
            max_tickets,
            tickets_issued: 0,
            status: EventStatus::Upcoming,
        };

        Storage::set_event(&env, event_id, &event);
        Storage::set_config(&env, &config);

        env.events()
            .publish((symbol_short!("evt_creat"),), (event_id, name));

        Ok(event_id)
    }

    pub fn issue_ticket(
        env: Env,
        event_id: u64,
        recipient: Address,
        tier: TicketTier,
    ) -> Result<u64, EventTicketError> {
        let mut config = Storage::get_config(&env)?;
        config.admin.require_auth();

        let mut event = Storage::get_event(&env, event_id)?;

        if event.tickets_issued >= event.max_tickets {
            return Err(EventTicketError::MaxTicketsReached);
        }

        let token_id = config.next_token_id;
        config.next_token_id += 1;

        let ticket = EventTicket {
            token_id,
            event_id,
            holder: recipient.clone(),
            tier: tier.clone(),
            transferable: true,
            attended: false,
            issued_at: env.ledger().timestamp(),
        };

        event.tickets_issued += 1;

        Storage::set_ticket(&env, token_id, &ticket);
        Storage::set_event(&env, event_id, &event);
        Storage::set_config(&env, &config);
        Storage::add_ticket_to_holder(&env, &recipient, token_id);

        env.events()
            .publish((symbol_short!("tkt_issu"),), (token_id, event_id, recipient, tier));

        Ok(token_id)
    }

    pub fn transfer_ticket(
        env: Env,
        token_id: u64,
        new_holder: Address,
    ) -> Result<(), EventTicketError> {
        let mut ticket = Storage::get_ticket(&env, token_id)?;
        let event = Storage::get_event(&env, ticket.event_id)?;

        if !ticket.transferable {
            return Err(EventTicketError::TicketNotTransferable);
        }

        let current_time = env.ledger().timestamp();
        if current_time >= event.start_at {
            return Err(EventTicketError::EventAlreadyStarted);
        }

        ticket.holder.require_auth();

        let old_holder = ticket.holder.clone();
        ticket.holder = new_holder.clone();

        Storage::set_ticket(&env, token_id, &ticket);
        Storage::remove_ticket_from_holder(&env, &old_holder, token_id);
        Storage::add_ticket_to_holder(&env, &new_holder, token_id);

        env.events()
            .publish((symbol_short!("tkt_tran"),), (token_id, old_holder, new_holder));

        Ok(())
    }

    pub fn check_in(env: Env, token_id: u64) -> Result<(), EventTicketError> {
        let config = Storage::get_config(&env)?;
        config.oracle.require_auth();

        let mut ticket = Storage::get_ticket(&env, token_id)?;
        let event = Storage::get_event(&env, ticket.event_id)?;

        let current_time = env.ledger().timestamp();
        if current_time < event.start_at {
            return Err(EventTicketError::EventNotStarted);
        }

        if current_time > event.end_at {
            return Err(EventTicketError::EventEnded);
        }

        if ticket.attended {
            return Err(EventTicketError::AlreadyCheckedIn);
        }

        ticket.attended = true;
        ticket.transferable = false;

        Storage::set_ticket(&env, token_id, &ticket);
        Storage::increment_attendance(&env, ticket.event_id);

        env.events()
            .publish(
                (symbol_short!("attend"),),
                (ticket.event_id, token_id, ticket.holder.clone()),
            );

        Ok(())
    }

    pub fn get_tickets(env: Env, holder: Address) -> Result<Vec<u64>, EventTicketError> {
        Ok(Storage::get_tickets_by_holder(&env, &holder))
    }

    pub fn get_attendance(env: Env, event_id: u64) -> Result<(u64, u64), EventTicketError> {
        Storage::get_event(&env, event_id)?;
        Ok(Storage::get_event_attendance(&env, event_id))
    }

    pub fn set_oracle(env: Env, new_oracle: Address) -> Result<(), EventTicketError> {
        let mut config = Storage::get_config(&env)?;
        config.admin.require_auth();
        config.oracle = new_oracle;
        Storage::set_config(&env, &config);
        Ok(())
    }
}

mod test;
