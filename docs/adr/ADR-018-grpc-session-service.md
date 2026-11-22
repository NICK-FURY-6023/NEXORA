# ADR-018: gRPC session service

## Context
Internal service-to-service RPC needs a typed contract independent of public REST.

## Decision
Define `nexora.v1.SessionService` in protobuf and serve it with tonic/prost, sharing the HTTP session manager.

## Consequences
Typed internal API and generated clients. Build requires vendored protoc, already pinned as a build dependency.

## Alternatives
Reusing REST internally was rejected because it would not provide typed streaming contracts.

## Status
Accepted and implemented in Phase 4b.
