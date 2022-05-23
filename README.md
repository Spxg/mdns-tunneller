# mDNS Over VPN
Use to forward multicast DNS packet. 

For example, you can control smart devices outside without HomePod, iPad and AppleTV.

But you still need a LAN environment that can connect to your home network.

## How To Work
Just forward multicast DNS packet to remote LAN.

## How To Use
* Clone this
* Select domains you want to forward
* Edit source config in `src/config.rs`
```rust
pub fn get_filter_domains() -> Vec<String> {
    vec![
        "_homekit._tcp.local".into(),
        "_hap._tcp.local".into(),
        "_googlecast._tcp.local".into()
    ]
}
```

* Run Server
```sh
cargo run server -a ip:port -i interface
```

* Run Client
```sh
cargo run client -a ip:port -i interface
```

_**In fact, there are no Server and Clients, query and answer packcet forward to each peer.**_
