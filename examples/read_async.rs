//            DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE
//                    Version 2, December 2004
//
// Copyleft (ↄ) meh. <meh@schizofreni.co> | http://meh.schizofreni.co
//
// Everyone is permitted to copy and distribute verbatim or modified
// copies of this license document, and changing it is allowed as long
// as the name is changed.
//
//            DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE
//   TERMS AND CONDITIONS FOR COPYING, DISTRIBUTION AND MODIFICATION
//
//  0. You just DO WHAT THE FUCK YOU WANT TO.

use std::thread::{sleep, Builder};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;
use tun::{AbstractDevice, BoxError};

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    main_entry().await?;
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

async fn main_entry() -> Result<(), BoxError> {
    let mut config = tun::Configuration::default();

    config
        .address((10, 0, 0, 9))
        .netmask((255, 255, 255, 0))
        .destination((10, 0, 0, 1))
        .mtu(tun::DEFAULT_MTU)
        .up();

    #[cfg(target_os = "linux")]
    config.platform_config(|config| {
        config.ensure_root_privileges(true);
    });

    let mut dev = tun::create_as_async(&config)?;
    let size = dev.mtu()? as usize + tun::PACKET_INFORMATION_LENGTH;
    let mut buf = vec![0; size];
    loop {
        tokio::select! {
            len = dev.read(&mut buf) => {
                println!("pkt: {:?}", &buf[..len?]);
            }
        };
    }
    Ok(())
}