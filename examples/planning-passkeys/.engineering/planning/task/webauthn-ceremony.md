---
format: aep.planning-md/1
id: task:webauthn-ceremony
kind: task
status: implemented
title: Implement the WebAuthn registration ceremony
owner: identity
relations:
  - decomposes: story:passkey-registration
revision: 5
---
# Task: Implement the WebAuthn registration ceremony

## What

Issue the creation challenge, verify the attestation object that comes back, and persist the
credential with its AAGUID, public key, sign count and transports.

## Why

`story:passkey-registration` is the ceremony plus the account page around it. This is the ceremony.

## Done When

Registration completes against a platform authenticator and a roaming key in the integration suite,
and an attestation with a challenge that was not issued is refused.

## Notes

The challenge store is the part that bites: it has to be single-use and expire, or a captured
registration response can be replayed into a second credential.
