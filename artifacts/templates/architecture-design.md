<!-- Starting point for an `architecture-design` artifact. Section names match
     `artifacts/kinds/architecture-design.yaml`. It is a design, so every design section still applies;
     the additional ones exist because the scope crosses ownership boundaries. -->

# Architecture design: <name>

## Context

*The decision or pressure that makes an architectural change necessary.*

## System Context

*The systems, teams and external parties in scope, and who talks to whom.*

## Goals

*The architectural outcomes sought, in terms someone outside the team can check.*

## Non-Goals

*Adjacent problems this does not solve, so nobody plans on it.*

## Current State

*Today's boundaries, dependencies and known violations of them.*

## Target vs Current State

*The delta, and the order in which it is closed. Name anything that will stay wrong for a while.*

## Proposed Design

*The target architecture: components, responsibilities and communication patterns.*

## Boundaries

*Where responsibility ends: service, persistence, security and failure-domain boundaries.*

## Ownership

*Which team owns each component and each dataset after this change.*

## Cross-System Invariants

*What must hold across systems, and which component enforces it. An invariant nobody enforces is a
wish.*

## Interfaces

*Contracts between the systems in scope, versioning, and how breakage is prevented.*

## Data Model

*Data ownership, duplication, and which store is authoritative for what.*

## Failure Modes

*Failures that cross a boundary, blast radius, and degradation behaviour.*

## Security

*Trust boundaries, authentication between systems, and the effect of compromising each one.*

## Observability

*Signals that span systems: tracing, correlation, and who is paged.*

## Migration

*How systems move without a synchronised cutover, and how long the dual state lasts.*

## Rollout

*Sequencing across teams and environments, with the checkpoint at each stage.*

## Rollback

*What is reversible, what is not, and the last reversible point.*

## Open Questions

*Unresolved cross-team questions, each with an owner.*
