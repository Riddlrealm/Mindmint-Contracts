# Example invocation

How to call a deployed contract via the Soroban CLI.

```bash
# Inspect
soroban contract inspect \
  --id <CONTRACT_ID> \
  --network testnet

# Read-only method
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source deployer \
  --network testnet \
  -- get_admin

# State-changing method (requires signing)
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source deployer \
  --network testnet \
  -- mint_certificate \
  --owner <PLAYER> \
  --puzzle_id '""' \
  --puzzle_title '""' \
  --completion_time_secs 0 \
  --rank 0 \
  --solution_hash '""' \
  --metadata_uri '""' \
  --transferable true
```
