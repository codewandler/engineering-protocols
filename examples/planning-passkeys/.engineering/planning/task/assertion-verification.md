---
format: aep.planning-md/1
id: task:assertion-verification
kind: task
status: active
title: Verify a sign-in assertion
owner: identity
relations:
  - decomposes: story:passkey-login
revision: 2
---
# Task: Verify a sign-in assertion

## What

Verify the assertion signature against the stored public key, check the origin and the relying party
id, and move the stored sign count forward.

## Why

`story:passkey-login` is sign-in end to end. This is the half that decides whether the person
holding the device is the person the credential belongs to.

## Done When

A valid assertion signs the user in; a replayed one, one from another origin, and one whose sign
count did not advance are each refused, and each refusal is recorded.

## Notes

Some authenticators always report a sign count of zero. Zero means "not counting" and must not be
read as "went backwards" — that check has to be conditional on the stored count being non-zero.
