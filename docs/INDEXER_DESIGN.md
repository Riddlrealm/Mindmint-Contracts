# Indexer design

Our indexer subscribes to RPC event streams, decodes them using generated schemas, and stores them in Postgres for UI.

Backpressure: indexer falls behind ⇒ emit a SEV3 page.
