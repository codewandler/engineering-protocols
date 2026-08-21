---
format: aep.planning-md/1
id: epic:passkey-sign-in
kind: epic
status: active
title: Passkey sign-in
summary: WebAuthn registration, sign-in and recovery for end users.
owner: identity
tags:
  - identity
relations:
  - decomposes: initiative:passwordless-authentication
revision: 6
---
# Epic: Passkey sign-in

## Outcome

An end user registers a passkey on one device, signs in with it, and can get back into their account
from a device that does not have it yet — without a password at any point.

## Why Now

The initiative cannot retire the password until all three of those exist. Shipping registration
alone would leave every account with a password still on it, which is the credential the initiative
is trying to remove.

## Scope

The WebAuthn ceremonies, credential storage, and the recovery flow that replaces "email me a reset
link". Three stories, decomposing this one.

## Out of Scope

Enterprise SSO, which is a different identity provider, and passkey sync between vendors, which is
the platform's job and not ours.

## Risks

A user with exactly one passkey and one lost device is locked out. `story:passkey-recovery` exists
to make that survivable, and it is why this epic does not finish without it.

## Done When

All three stories are `implemented` and the password field is gone from the sign-in form.
