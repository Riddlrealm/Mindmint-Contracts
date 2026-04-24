# NFT Trait and Attribute Trading Marketplace

A decentralized marketplace for trading individual NFT traits and attributes. Instead of trading whole NFTs, players can list specific traits from their NFTs for sale. Buyers purchase traits and have them transferred to their own NFTs, creating a dynamic trait economy.

## Features

- **Trait-Level Trading**: Trade individual traits instead of entire NFTs
- **Source-to-Destination Transfer**: Traits are removed from source NFT and added to destination NFT
- **Configurable Platform Fees**: Flexible fee structure (basis points)
- **Trait Validation**: Prevents duplicate traits and ensures ownership
- **Trait Discovery**: Browse active listings by trait type
- **Cross-Contract Integration**: Ready for NFT contract integration
- **Event Emission**: Full event tracking for indexing

## Architecture

### Data Structures

#### TraitListing
```rust
pub struct TraitListing {
    pub id: u32,                      // Unique listing identifier
    pub seller: Address,              // Address listing the trait
    pub source_nft_id: u32,           // NFT to remove trait from
    pub trait_key: String,            // Trait name/identifier
    pub trait_value: String,          // Trait value
    pub price: i128,                  // Price in payment token
    pub payment_token: Address,       // Token address for payment
    pub status: u8,                   // 0 = active, 1 = sold, 2 = cancelled
    pub created_at: u64,              // Listing creation timestamp
}
```

#### MarketplaceConfig
```rust
pub struct MarketplaceConfig {
    pub admin: Address,               // Contract administrator
    pub platform_fee_bps: u32,        // Platform fee in basis points
    pub fee_collector: Address,       // Address receiving platform fees
    pub nft_contract: Address,        // NFT contract address
    pub payment_token: Address,       // Token used for payments
}
```

### Core Functions

#### `initialize(admin, platform_fee_bps, nft_contract, payment_token)`
Initializes the marketplace. Can only be called once.

**Parameters:**
- `admin`: Marketplace administrator
- `platform_fee_bps`: Platform fee in basis points (max 10,000 = 100%)
- `nft_contract`: Address of NFT contract
- `payment_token`: Token address for trait purchases

**Validation:**
- Admin must provide auth
- Fee must not exceed 10,000 bps

#### `list_trait(seller, source_nft_id, trait_key, trait_value, price)`
Lists a trait for sale.

**Parameters:**
- `seller`: Address listing the trait (must provide auth)
- `source_nft_id`: NFT ID to remove trait from
- `trait_key`: Trait identifier (e.g., "color", "rarity")
- `trait_value`: Trait value (e.g., "blue", "epic")
- `price`: Price in payment tokens

**Validation:**
- Price must be positive
- Trait cannot already be listed on this NFT
- Seller must own source NFT (enforced by caller)

**Returns:** Listing ID

**Events:**
- `TraitListed(listing_id, seller, source_nft_id, trait_key, price)`

#### `buy_trait(listing_id, buyer, destination_nft_id)`
Purchases a listed trait and transfers it to buyer's NFT.

**Parameters:**
- `listing_id`: ID of the listing to purchase
- `buyer`: Address purchasing trait (must provide auth)
- `destination_nft_id`: NFT to receive the trait

**Validation:**
- Listing must be active
- Destination NFT must not already have this trait
- Buyer must have sufficient token balance
- Buyer must own destination NFT (enforced by caller)

**Processing:**
1. Calculate platform fee
2. Transfer seller amount to seller
3. Transfer fee to fee collector
4. Remove trait from source NFT
5. Add trait to destination NFT
6. Mark listing as sold

**Events:**
- `TraitSold(listing_id, buyer, destination_nft_id, seller, price, fee)`

#### `cancel_listing(seller, listing_id)`
Cancels a trait listing. Only seller can cancel.

**Parameters:**
- `seller`: Original trait seller (must provide auth)
- `listing_id`: ID of listing to cancel

**Validation:**
- Listing must be active
- Caller must be the original seller

**Side Effects:**
- Trait is unmarked as listed on source NFT
- Listing status set to cancelled

**Events:**
- `ListingCancelled(listing_id, seller, source_nft_id, trait_key)`

#### `get_listing(listing_id) -> Option<TraitListing>`
Retrieves listing details.

**Returns:** TraitListing struct or None

#### `has_trait(nft_id, trait_key) -> bool`
Checks if NFT has a specific trait.

**Returns:** Boolean indicating trait presence

#### `get_trait(nft_id, trait_key) -> Option<String>`
Gets the value of a trait on an NFT.

**Returns:** Trait value string or None

#### `list_active_listings(trait_key) -> Vec<TraitListing>`
Returns all active listings for a trait type.

**Parameters:**
- `trait_key`: Trait type to filter by

**Returns:** Vector of active TraitListing structs

#### `get_seller_active_listings(seller) -> Vec<TraitListing>`
Returns seller's active listings.

**Returns:** Vector of TraitListing structs

#### `update_platform_fee(admin, new_fee_bps)`
Updates platform fee. Admin only.

**Parameters:**
- `admin`: Contract admin (must provide auth)
- `new_fee_bps`: New fee in basis points

**Validation:**
- Caller must be admin
- Fee must not exceed 10,000 bps

**Events:**
- `FeeUpdated(new_fee_bps)`

#### `update_fee_collector(admin, new_collector)`
Updates fee collector address. Admin only.

**Parameters:**
- `admin`: Contract admin (must provide auth)
- `new_collector`: New collector address

**Events:**
- `CollectorUpdated(new_collector)`

## Fee Structure

Platform fees are deducted from the listing price:

```
Seller Amount = Price - Fee
Fee = (Price × Platform_Fee_BPS) / 10,000
```

**Examples:**

| Price | Fee BPS | Fee Amount | Seller Receives |
|-------|---------|-----------|-----------------|
| 1,000 | 0 | 0 | 1,000 |
| 1,000 | 250 | 25 | 975 |
| 1,000 | 500 | 50 | 950 |
| 1,000 | 1,000 | 100 | 900 |
| 1,000 | 2,500 | 250 | 750 |

## Events

### TraitListed
Emitted when a trait is listed for sale.
```
Event: ("nft_trait", "trait_listed")
Data: (listing_id, seller, source_nft_id, trait_key, price)
```

### TraitSold
Emitted when a trait is successfully purchased.
```
Event: ("nft_trait", "trait_sold")
Data: (listing_id, buyer, destination_nft_id, seller, price, fee)
```

### ListingCancelled
Emitted when a listing is cancelled.
```
Event: ("nft_trait", "listing_cancelled")
Data: (listing_id, seller, source_nft_id, trait_key)
```

### FeeUpdated
Emitted when platform fee changes.
```
Event: ("nft_trait", "fee_updated")
Data: new_fee_bps
```

### CollectorUpdated
Emitted when fee collector address changes.
```
Event: ("nft_trait", "collector_updated")
Data: new_collector
```

## Trait Management

### Trait Storage
- Traits are stored as key-value pairs on NFTs
- Each trait has a `trait_key` (identifier) and `trait_value` (data)
- Common trait keys: "color", "rarity", "element", "background", "accessory"

### Trait Lifecycle

1. **Listed**: Trait is marked as listed on source NFT (cannot be re-listed)
2. **Sold**: Trait is transferred to destination NFT (removed from source)
3. **Owned**: Trait is now property of destination NFT owner
4. **Re-listed**: New owner can list the trait again

### Trait Transfer Process

```
Source NFT (owns trait)
    ↓ [buyer purchases]
    ↓ [platform fee deducted]
    ↓ [seller paid]
    ↓ [cross-contract call]
    ↓
Destination NFT (receives trait)
```

## Testing

Comprehensive test suite included:

```rust
#[test]
fn test_list_trait()               // Listing creation
#[test]
fn test_buy_trait()                // Trait purchase and transfer
#[test]
fn test_cancel_listing()           // Listing cancellation
#[test]
fn test_duplicate_trait_rejection() // Prevents duplicate listings
#[test]
fn test_fee_deduction()            // Verifies fee calculation
#[test]
fn test_list_active_listings()     // Trait type filtering
#[test]
fn test_destination_trait_rejection() // Prevents duplicate traits
```

### Running Tests

```bash
cd contracts/nft_trait_marketplace
cargo test
```

## Integration Examples

### Listing a Trait

```rust
// Seller lists their NFT's trait
let listing_id = marketplace_client.list_trait(
    &seller_address,
    &1u32,  // source NFT ID
    &String::from_slice(&env, "color"),
    &String::from_slice(&env, "gold"),
    &5000i128,  // price in payment tokens
);
```

### Purchasing a Trait

```rust
// Buyer purchases trait from listing
marketplace_client.buy_trait(
    &listing_id,
    &buyer_address,
    &7u32,  // destination NFT ID
);
// Trait now belongs to buyer's NFT #7
```

### Browsing Listings

```rust
// Get all active listings for "color" trait
let color_listings = marketplace_client.list_active_listings(
    &String::from_slice(&env, "color")
);

// Get specific seller's listings
let seller_listings = marketplace_client.get_seller_active_listings(&seller_address);
```

## Cross-Contract Integration

For full integration with an NFT contract:

1. **Ownership Verification**: Call NFT contract to verify ownership
2. **Trait Application**: Call NFT contract to add/remove traits
3. **Event Coordination**: Ensure trait changes trigger NFT events

### Example Call Pattern

```rust
// In buy_trait function
let nft_client = nft::Client::new(&env, &config.nft_contract);

// Remove trait from source
nft_client.remove_trait(&listing.source_nft_id, &listing.trait_key);

// Add trait to destination
nft_client.add_trait(
    &destination_nft_id,
    &listing.trait_key,
    &listing.trait_value,
);
```

## Security Considerations

1. **Authorization**: All state-changing functions require caller auth
2. **Ownership Verification**: Assumes caller verification (seller owns source, buyer owns destination)
3. **Trait Uniqueness**: Prevents duplicate traits on single NFT
4. **Fee Bounds**: Platform fee capped at 100%
5. **Status Validation**: Prevents operations on sold/cancelled listings
6. **Amount Validation**: Price must be positive

## Deployment

1. Deploy contract to Stellar network
2. Call `initialize` with configuration
3. Configure NFT contract address
4. Set up fee collector wallet
5. Distribute marketplace UI to users

## Marketplace Economics

### For Sellers
- Earn revenue by trading rare/valuable traits
- Customize NFTs by selling unwanted traits
- Trait prices determined by market demand

### For Buyers
- Acquire specific traits without buying whole NFTs
- Compose custom NFT sets
- Lower entry costs for desired traits

### For Platform
- Collect fees on every trait transaction
- Encourage marketplace activity through discovery
- Enable community-driven trait economy

## Limitations & Future Improvements

- Trait proof of originality not enforced (trust in NFT contract)
- No batch operations for gas optimization
- No trait insurance or dispute resolution
- No trait expiration/degradation over time
- Consider implementing:
  - Auction system for traits
  - Trait rentals (temporary transfers)
  - Trait bundles (multiple traits at discount)
  - Royalties for trait creators
  - Rarity-based fee tiers
  - Trait ratings/reviews
