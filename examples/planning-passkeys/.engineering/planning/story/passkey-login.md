---
format: aep.planning-md/1
id: story:passkey-login
kind: story
status: active
title: Sign in with a passkey
summary: A returning user signs in with a passkey instead of a password.
owner: identity
tags:
  - webauthn
relations:
  - decomposes: epic:passkey-sign-in
  - depends_on: story:passkey-registration
revision: 7
---
# Story: Sign in with a passkey

## Outcome

A returning user is signed in by their device: no password typed, no code copied out of an email.

## Context

Depends on registration, because there is nothing to assert against until an account has a
credential. The password field stays on the form while this is in flight — removing it is the
initiative's last step, not this story's.

## Acceptance

- The assertion is verified against the stored public key, and a replayed assertion is refused.
- A sign count that goes backwards is treated as a cloned authenticator and refused, with the
  attempt recorded.
- A user with two passkeys may sign in with either.
- The refusal a user sees never says which of the two halves failed.

## Open Questions

- Whether to offer conditional UI (autofill) in the first release. Decides: identity. The ceremony
  is the same either way, so this does not block the work.
