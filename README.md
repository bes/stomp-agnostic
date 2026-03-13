# STOMP Agnostic

[![crates.io](https://img.shields.io/crates/v/stomp-agnostic.svg)](https://crates.io/crates/stomp-agnostic)
[![docs.rs](https://docs.rs/stomp-agnostic/badge.svg)](https://docs.rs/stomp-agnostic/latest/stomp_agnostic/)

A transport and async agnostic [STOMP](https://stomp.github.io/) library for Rust.

This is a fork of [async-stomp](https://github.com/snaggen/async-stomp).

## Overview

This library contains an implementation of the STOMP 1.2 protocol, but does not
mandate any specific transport method - bring your own!

## (Non-) Performance
This crate does not have a specific focus on performance.

## Transport agnostic
Other STOMP libraries, like [async-stomp](https://github.com/snaggen/async-stomp),
[wstomp](https://crates.io/crates/wstomp), etc. focus on one, or a few, specific transport
methods such as TCP or WebSockets. This crate on the other hand, exposes two traits,
[ClientTransport] and [ServerTransport] and the implementor is responsible for the transport.
This makes this crate compatible with e.g. [tokio-tungstenite](https://crates.io/crates/tokio-tungstenite),
but you have to implement the `Transport` trait yourself, there is nothing implemented for
`tokio-tungstenite` out-of-the box.

## Async agnostic
This crate does not depend on a specific async stack. Bring your own.

## High level STOMP interface and low level control of the transport
`ClientStompHandle` and `ServerStompHandle` are the high-level APIs to use when sending and
receiving STOMP messages, but `stomp-agnostic` also makes it easy to get a hold of the
underlying transport implementation, both as an exclusive refernce `&mut T` and consuming the
handle itself to get the original `T: ClientTransport` or `T: ServerTransport` back, to perform
low-level cleanup at any time, usually at the end of a session. This is accomplished through
[ClientStompHandle::into_transport], [ClientStompHandle::as_mut_transport],
[ServerStompHandle::into_transport], and [ServerStompHandle::as_mut_transport].

## Examples
There are two examples: one implementing a basic WebSocket STOMP client using
`tokio-tungstenite`, and another implementing a basic WebSocket STOMP server using `axum`.

## Features

- Async STOMP server and client for Rust
- Support for all STOMP operations:
  - Connection management (connect, disconnect)
  - Message publishing
  - Subscriptions
  - Acknowledgments (auto, client, client-individual modes)
  - Transactions
- Custom headers support for advanced configurations

## License

Licensed under the [EUPL](LICENSE.EUPL).
