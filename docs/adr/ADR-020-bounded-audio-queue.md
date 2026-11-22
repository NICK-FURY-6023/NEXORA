# ADR-020: Bounded audio queues

## Context
Unbounded queues can exhaust memory under backpressure or slow clients.

## Decision
Use bounded channels with explicit `try_push` rejection and backpressure-oriented APIs.

## Consequences
Overloaded producers receive errors instead of growing memory without limit. Callers must handle full queues.

## Alternatives
Unbounded `VecDeque` was rejected because it hides overload.

## Status
Accepted; `BoundedAudioQueue` implemented in Phase 2.
