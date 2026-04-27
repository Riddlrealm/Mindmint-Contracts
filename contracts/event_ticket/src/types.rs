use soroban_sdk::{contracterror, contracttype, Address, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EventTicketError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    EventNotFound = 4,
    TicketNotFound = 5,
    EventNotStarted = 6,
    EventAlreadyStarted = 7,
    EventEnded = 8,
    MaxTicketsReached = 9,
    TicketNotTransferable = 10,
    NotTicketHolder = 11,
    AlreadyCheckedIn = 12,
    InvalidTier = 13,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TicketTier {
    General = 0,
    VIP = 1,
    Backstage = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventTicket {
    pub token_id: u64,
    pub event_id: u64,
    pub holder: Address,
    pub tier: TicketTier,
    pub transferable: bool,
    pub attended: bool,
    pub issued_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub id: u64,
    pub name: Symbol,
    pub start_at: u64,
    pub end_at: u64,
    pub max_tickets: u64,
    pub tickets_issued: u64,
    pub status: EventStatus,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventStatus {
    Upcoming = 0,
    Active = 1,
    Ended = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub oracle: Address,
    pub next_event_id: u64,
    pub next_token_id: u64,
}
