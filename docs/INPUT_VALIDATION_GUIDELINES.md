# Input validation guidelines

Every public function should validate its arguments in this order:

1. **Type** — argument is well-formed for its declared type.
2. **Range** — numeric bounds; non-negative; non-zero where required.
3. **Length** — bounded string, vector, and map sizes.
4. **Authentication** — `require_auth()`.
5. **Authorization** — caller is permitted.

Validation failures panic with the documented `Error::*` variant.
