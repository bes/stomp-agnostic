# STOMP Agnostic

[![crates.io](https://img.shields.io/crates/v/stomp-agnostic.svg)](https://crates.io/crates/stomp-agnostic)
[![docs.rs](https://docs.rs/stomp-agnostic/badge.svg)](https://docs.rs/async-stomp/latest/stomp_agnostic/)

A transport and async agnostic [STOMP](https://stomp.github.io/) library for Rust.

This is a fork of [async-stomp](https://github.com/snaggen/async-stomp).

## Overview

This library contains an implementation of the STOMP 1.2 protocol, but does not
mandate any specific transport method.

# (Non-) Performance
This crate does not have a specific focus on performance.

# Transport agnostic
Other STOMP libraries, like [async-stomp](https://github.com/snaggen/async-stomp),
[wstomp](https://crates.io/crates/wstomp), etc. focus on one, or a few, specific transport
methods such as TCP or WebSockets. This crate on the other hand, exposes a trait `Transport`
and the implementor is responsible for the transport. This makes this crate compatible with
e.g. [tokio-tungstenite](https://crates.io/crates/tokio-tungstenite), but you have to implement
the `Transport` trait yourself, there is nothing implemented for `tokio-tungstenite` out-of-the box.

# Async agnostic
This crate does not depend on a specific async stack. Bring your own.

## Features

- Async STOMP client for Rust
- Support for all STOMP operations:
  - Connection management (connect, disconnect)
  - Message publishing
  - Subscriptions
  - Acknowledgments (auto, client, client-individual modes)
  - Transactions
- Custom headers support for advanced configurations

## License

Licensed under the [EUPL](LICENSE.EUPL).
