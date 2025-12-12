use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct DeviceConfig {
    pub name: String,
    pub ip: String,
    pub mask: String,
    pub gateway: String,
    pub mtu: usize,
    pub routes: Vec<String>,
}

pub struct Device {
    #[allow(dead_code)]
    config: DeviceConfig,
    inbound_tx: mpsc::Sender<Vec<u8>>,
    outbound_rx: mpsc::Receiver<Vec<u8>>,
}

impl Device {
    pub fn new(config: DeviceConfig,
               inbound_tx: mpsc::Sender<Vec<u8>>,
               outbound_rx: mpsc::Receiver<Vec<u8>>) -> Self {
        Self { config, inbound_tx, outbound_rx }
    }

    pub async fn run(&mut self) -> crate::Result<()> {
        let mut config = tun::Configuration::default();
        config
            .address(self.config.ip.clone())
            .netmask(self.config.mask.clone())
            .destination(self.config.gateway.clone())
            .mtu(self.config.mtu as u16)
            .up();

        #[cfg(target_os = "linux")]
        config.platform_config(|config| {
            config.ensure_root_privileges(true);
        });

        let mut dev = match tun::create_as_async(&config){
            Ok(dev) => dev,
            Err(e) => {
                return Err(e.into());
            }
        };

        let mut buf = vec![0; 2048];
        loop {
            tokio::select! {
                amount = dev.read(&mut buf) => {
                    let amount = match amount {
                        Ok(amount) => amount,
                        Err(e) => {
                            tracing::error!("read device fail: {:?}", e);
                            continue;
                        }
                    };
                    if let Err(e) = self.inbound_tx.send(buf[0..amount].to_vec()).await {
                        tracing::error!("device => server fail: {}", e);
                    }
                }
                packet = self.outbound_rx.recv() => {
                    if let Some(packet) = packet {
                        tracing::info!("server => device {} bytes", packet.len());
                        let result = dev.write(packet.as_slice()).await;
                        if let Err(e) = result {
                            tracing::error!("write device fail: {:?}", e);
                        }
                    }
                }
            }
        }
    }
}

pub struct DeviceHandler {
    inbound_rx: Option<mpsc::Receiver<Vec<u8>>>,
    outbound_tx: Option<mpsc::Sender<Vec<u8>>>,
}

impl DeviceHandler {
    pub fn new() -> Self {
        Self{inbound_rx: None, outbound_tx: None}
    }

    pub fn run(&mut self, cfg: DeviceConfig) -> crate::Result<()> {
        let (inbound_tx, inbound_rx) = mpsc::channel(1000);
        let (outbound_tx, outbound_rx) = mpsc::channel(1000);
        self.inbound_rx = Some(inbound_rx);
        self.outbound_tx = Some(outbound_tx);

        let mut dev = Device::new(cfg, inbound_tx, outbound_rx);
        tokio::spawn(async move {
            let res = dev.run().await;
            match res {
                Ok(_) => (),
                Err(e) => tracing::error!("device handler fail: {:?}", e),
            }
        });

        Ok(())
    }

    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        let inbound_rx = match self.inbound_rx.as_mut() {
            Some(rx) => rx,
            None => {
                tracing::error!("device handler recv none");
                return None;
            },
        };

        inbound_rx.recv().await
    }

    pub async fn send(&self, packet: Vec<u8>) -> crate::Result<()> {
        let outbound_tx = match self.outbound_tx.as_ref() {
            Some(tx) => tx,
            None => {
                return Err("device handler send none".into());
            }
        };
        tracing::info!("device => server outbound tx len: {}", packet.len());
        let result = outbound_tx.send(packet).await;
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

