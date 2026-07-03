# JavaScript SDK integration

```ts
import { Contract, Server } from '@stellar/stellar-sdk';
const server = new Server('https://soroban-testnet.stellar.org');
const c = new Contract(contractId);
const tx = await c.call('get_balance', ...);
```
