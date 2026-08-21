---
format: aep.planning-md/1
id: story:passkey-recovery
kind: story
status: proposed
title: Recover an account without the registered device
summary: Get back in after losing the only device holding a passkey.
owner: identity
tags:
  - webauthn
  - support
relations:
  - decomposes: epic:passkey-sign-in
  - depends_on: story:passkey-registration
revision: 3
---
# Story: Recover an account without the registered device

## Outcome

A user whose only passkey was on a phone that is now at the bottom of a canal gets back into their
account, and the path they take is not a password.

## Context

This is the story that decides whether the initiative can finish. As long as recovery falls back to
"email a reset link", the password is still a credential and phishing it still works — so the
account is only as strong as the mailbox.

## Acceptance

- Recovery is possible with no access to the previously registered device.
- The recovery path does not accept, set or require a password at any point.
- A completed recovery invalidates nothing that is still in the user's possession.
- Every recovery is recorded with what was presented, because this is the flow an attacker will
  aim at.

## Open Questions

- Which second factor stands in: a second registered passkey, an identity document check, or a
  support-verified path with a delay. Decides: identity, with security. This is what keeps the
  story `proposed` rather than picked up.
