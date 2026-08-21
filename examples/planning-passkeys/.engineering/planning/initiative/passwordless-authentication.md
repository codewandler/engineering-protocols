---
format: aep.planning-md/1
id: initiative:passwordless-authentication
kind: initiative
status: active
title: Passwordless authentication
summary: Retire the password as a primary credential.
owner: identity
tags:
  - identity
  - security
revision: 4
---
# Initiative: Passwordless authentication

## Outcome

Nobody signing in has to remember anything. The password stops being the credential that gets
phished, reused across sites and reset by a support agent who cannot verify who they are talking to.

## Why Now

Credential stuffing is the single largest source of account takeover in this system, and every
mitigation so far has made signing in slower without making it safer. Platform authenticators are
now available on every device class we support, so the alternative finally exists.

## Scope

Sign-in and account recovery for end users. Employee access runs on a different identity provider
and is not touched here.

## Done When

Passwords are no longer accepted as a primary credential for end-user sign-in, and the recovery path
does not fall back to one.
