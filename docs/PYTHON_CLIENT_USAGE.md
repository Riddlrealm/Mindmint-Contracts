# Python client usage

```python
from stellar_sdk import SorobanServer
srv = SorobanServer('https://soroban-testnet.stellar.org')
res = srv.invoke_contract(contract_id, 'get_balance', ...)
```
