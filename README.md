# Birdwatcher-rs

Run periodic healthcheck on a service to automatically stop or start [BIRD](https://bird.network.cz/) advertisements.

Inspired by this [birdwatcher](https://github.com/skoef/birdwatcher) by Skoef.

## Usage

### With Nix
Quick start:
```
nix run github:pixelshot91/outputter -- --help
```

To use install it on NixOs, look up `integration_test/single_service.nix`


### Without Nix
1. [Install Rust](https://www.rust-lang.org/tools/install)
2. `cargo run -- --config my_config.toml`

## Configuration

```toml
generated_file_path = "birdwatcher_generated.conf"

[bird_reload]
command = ["birdc", "configure"]
timeout_s = 2

[[service_definitions]]
service_name = "webserver is up"
function_name = "webserver_is_active"
command = ["/bin/curl", "http://localhost:8000/"]
command_timeout_s = 2
interval_s = 1
fall = 1
rise = 3

[[service_definitions]]
service_name = "My important file exist"
function_name = "file_exist"
command = ["/bin/ls", "/root/my_file.txt"]
command_timeout_s = 1
interval_s = 5
fall = 1
rise = 3
```

## Why fork Skoef's birdwatcher

Little improvements over Skoef's birdwatcher:
  - `birdwatcher-rs` does not link a service with a list of IP addresses like `birdwatcher` does.  
  `birdwatcher-rs` only return a function return a simple `true` or `false`, whereas `birdwatcher` force to use a BIRD filter, which might prevent some configuration, and caused some problems.  
  See https://github.com/skoef/birdwatcher/issues/28 and https://github.com/skoef/birdwatcher/issues/25
  - I tried to make `birdwatcher-rs` code simpler. The only moving part is a list of value representing the service states.
  - `birdwatcher-rs` enable the service if the first check is successful, whatever the `rise` value is.
  - It uses Rust, which I find cleaner and prevent entire classes of bugs compared to Go, such as null-pointer exception, generics, sum-type.
  - `birdwatcher-rs` support Nix, for easy and reproducible build. It include an integration test running birdwatcher-rs in a VM with NixOS test framekwork.

## Development
To build from source:
```
nix build
result/bin/birdwatcher-rs --config my_config.toml
```
