---
format: aep.planning-md/1
id: story:passkey-registration
kind: story
status: implemented
title: Register a passkey
summary: A signed-in user creates a passkey on the device they are using.
owner: identity
tags:
  - webauthn
relations:
  - decomposes: epic:passkey-sign-in
revision: 9
---
# Story: Register a passkey

## Outcome

A signed-in user can create a passkey on the device they are holding, and see it listed among their
credentials afterwards with a name they recognise.

## Context

This is the first of the three, because nothing else can be tested until an account has a passkey on
it. It does not remove the password — that is the initiative's job, and only once recovery exists.

## Acceptance

- The registration ceremony completes on a platform authenticator and on a roaming key.
- The stored credential records its AAGUID, sign count and the transports the authenticator
  reported.
- Registering a second passkey on a second device does not invalidate the first.
- A user who abandons the ceremony half way is left with no partial credential.

## Out of Scope

Naming and deleting credentials from the account page — a follow-up, and not a blocker for sign-in.
