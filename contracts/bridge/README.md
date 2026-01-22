# Cross-Chain Asset Bridge Contract

This Soroban contract implements a secure cross-chain bridge for NFT and token transfers between Stellar and other blockchains, featuring validator-based security, fee collection, and emergency controls.

## Bridge Architecture

### Security Model
- **Multi-signature validation**: Requires threshold of validator signatures for cross-chain operations
- **Reentrancy protection**: Prevents recursive calls during asset transfers
- **Emergency pause**: Administrative controls to halt operations in emergencies
- **Replay attack prevention**: Nonce-based message uniqueness
- **Asset locking**: Assets are securely locked before cross-chain transfer

### Core Components

#### 1. Bridge Messages
```rust
BridgeMessage {
    message_id: BytesN<32>,      // Unique message identifier
    source_chain: u32,           // Source blockchain ID (0 = Stellar)
    dest_chain: u32,             // Destination blockchain ID
    action: BridgeAction,        // Lock/Unlock operation
    asset_type: AssetType,       // Token or NFT
    asset_address: Address,      // Asset contract address
    asset_amount: i128,          // Amount or token ID
    sender: Address,             // Source chain sender
    recipient: Bytes,            // Destination chain recipient
    fee_amount: i128,            // Bridge fee
    timestamp: u64,              // Message creation time
    nonce: u64,                  // Anti-replay nonce
}
```

#### 2. Validator System
- **Dynamic validator set**: Admin can add/remove validators
- **Signature threshold**: Configurable number of required signatures
- **Version control**: Validator set versioning prevents signature replay

#### 3. Asset Handling

##### Token Bridging
- **Lock**: Transfer tokens from user to bridge contract
- **Unlock**: Release tokens to recipient on destination chain
- **Fee collection**: Automatic fee deduction in bridge operations

##### NFT Bridging
- **Wrapping**: Convert external NFTs to bridge-compatible format
- **Unwrapping**: Restore original NFT after cross-chain transfer
- **Metadata preservation**: Maintain NFT attributes across chains

## Bridge Operations

### 1. Asset Locking (Initiate Bridge)
```rust
bridge_assets(sender, asset_address, asset_type, amount, dest_chain, recipient)
```
- Locks assets in bridge contract
- Generates unique message ID
- Calculates and collects bridge fees
- Emits `BRIDGE_INIT` event

### 2. Cross-Chain Validation
- Validators sign bridge messages off-chain
- Signatures collected and submitted to destination chain
- Multi-signature verification ensures security

### 3. Asset Unlocking (Complete Bridge)
```rust
complete_bridge(validator, message, signatures)
```
- Verifies validator signatures meet threshold
- Unlocks assets to recipient address
- Updates message status to `Completed`
- Emits `BRIDGE_COMP` event

### 4. Emergency Operations
- **Cancel**: Users/admins can cancel pending bridges
- **Pause**: Admin can pause all bridge operations
- **Asset recovery**: Locked assets can be refunded

## Fee System

### Fee Calculation
```
fee = max(min_fee, min(max_fee, (amount * base_fee_bps) / 10000))
```

### Fee Parameters
- **Base fee**: 30 basis points (0.3%) default
- **Minimum fee**: 1 XLM (configurable)
- **Maximum fee**: 1M XLM (configurable)
- **Fee collector**: Designated address for fee accumulation

## Security Features

### Reentrancy Protection
- Asset transfers use checked arithmetic
- State changes before external calls
- Non-reentrant function modifiers

### Input Validation
- Asset amount validation (> 0)
- Chain ID bounds checking
- Address format verification
- Signature threshold enforcement

### Emergency Controls
- Contract-wide pause functionality
- Admin-only validator management
- Cancel operations for stuck transfers

## Validator Operations

### Validator Management
- **Add validator**: Admin can add authorized validators
- **Remove validator**: Admin can remove validators
- **Version tracking**: Validator set changes increment version

### Signature Verification
- **Ed25519 signatures**: Cryptographically secure validation
- **Threshold requirements**: Must meet required signature count
- **Validator authorization**: Only registered validators can sign

## Event Monitoring

### Bridge Events
- `BRIDGE_INIT`: Bridge operation initiated
- `BRIDGE_COMP`: Bridge operation completed
- `BRIDGE_CANCEL`: Bridge operation cancelled
- `BRIDGE_PAUSE`: Contract pause state changed

### Validator Events
- `VALIDATOR_ADD`: New validator added
- `VALIDATOR_REM`: Validator removed

### NFT Events
- `NFT_WRAP`: NFT wrapped for bridging
- `NFT_UNWRAP`: NFT unwrapped after bridging

## Chain ID Mapping

| Chain ID | Blockchain |
|----------|------------|
| 0 | Stellar |
| 1 | Ethereum |
| 2 | BSC |
| 3 | Polygon |
| ... | Other chains |

## Testing Coverage

### Unit Tests
- Bridge initialization and configuration
- Validator management operations
- Token bridging (lock/unlock cycle)
- NFT wrapping/unwrapping
- Fee calculation and collection
- Emergency pause functionality

### Integration Tests
- Multi-validator signature verification
- Cross-chain message validation
- Asset transfer security
- Reentrancy attack prevention

## Deployment Considerations

### Testnet Deployment
- Deploy with minimal validator set (1-3 validators)
- Test all bridge operations
- Verify fee collection
- Test emergency controls

### Mainnet Deployment
- Deploy with full validator set
- Multi-sig admin controls
- Comprehensive security audit
- Gradual feature rollout

## Future Enhancements

- **Light client verification**: On-chain proof validation
- **Batch bridging**: Multiple assets in single transaction
- **Dynamic fees**: Market-based fee adjustment
- **Cross-chain governance**: Validator voting mechanisms
- **Bridge analytics**: Transaction volume tracking

## Security Considerations

1. **Validator compromise**: Multi-sig requirements prevent single points of failure
2. **Replay attacks**: Nonce-based message uniqueness
3. **Asset loss**: Emergency recovery mechanisms
4. **Fee manipulation**: Capped fee ranges prevent exploitation
5. **Contract upgrades**: Pause functionality enables safe upgrades

## Performance Optimization

- **Lazy evaluation**: Message status checked on-demand
- **Efficient storage**: Minimal on-chain state
- **Gas optimization**: Batch operations where possible
- **Event filtering**: Structured event data for off-chain indexing

This bridge contract provides a secure, efficient, and extensible solution for cross-chain asset transfers in the Stellar ecosystem.