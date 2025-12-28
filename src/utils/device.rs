use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
#[allow(unused_imports)]
use tun::AbstractDevice;

#[derive(Clone)]
pub struct DeviceConfig {
    pub ip: String,
    pub mask: String,
    pub gateway: String,
    pub mtu: u16,
}

pub struct Device {
    #[allow(dead_code)]
    config: DeviceConfig,
    inbound_tx: mpsc::Sender<Vec<u8>>,
    outbound_rx: mpsc::Receiver<Vec<u8>>,
}

impl Device {
    pub fn new(
        config: DeviceConfig,
        inbound_tx: mpsc::Sender<Vec<u8>>,
        outbound_rx: mpsc::Receiver<Vec<u8>>,
    ) -> Self {
        Self {
            config,
            inbound_tx,
            outbound_rx,
        }
    }

    pub async fn run(&mut self, ready: oneshot::Sender<Option<i32>>) -> crate::Result<()> {
        let mut config = tun::Configuration::default();
        config
            .address(self.config.ip.clone())
            .netmask(self.config.mask.clone())
            // .destination(self.config.gateway.clone())
            .mtu(self.config.mtu)
            .up();

        #[cfg(target_os = "linux")]
        config.platform_config(|config| {
            config.ensure_root_privileges(true);
        });

        let mut dev = match tun::create_as_async(&config) {
            Ok(dev) => dev,
            Err(e) => {
                return Err(e.into());
            }
        };

        // Get TUN interface index (Windows only)
        #[cfg(target_os = "windows")]
        let tun_index = dev.tun_index().ok();
        
        #[cfg(not(target_os = "windows"))]
        let tun_index: Option<i32> = None;

        let _ = ready.send(tun_index);
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
                        tracing::debug!("server => device {} bytes", packet.len());
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
    pub rx_bytes: usize,
    pub tx_bytes: usize,
}

impl DeviceHandler {
    pub fn new() -> Self {
        Self {
            inbound_rx: None,
            outbound_tx: None,
            rx_bytes: 0,
            tx_bytes: 0,
        }
    }

    pub async fn run(&mut self, cfg: DeviceConfig) -> crate::Result<Option<i32>> {
        let (inbound_tx, inbound_rx) = mpsc::channel(1000);
        let (outbound_tx, outbound_rx) = mpsc::channel(1000);
        self.inbound_rx = Some(inbound_rx);
        self.outbound_tx = Some(outbound_tx);

        let mut dev = Device::new(cfg, inbound_tx, outbound_rx);
        let (ready_tx, ready_rx) = oneshot::channel();
        tokio::spawn(async move {
            let res = dev.run(ready_tx).await;
            match res {
                Ok(_) => (),
                Err(e) => tracing::error!("device handler fail: {:?}", e),
            }
        });

        let tun_index = ready_rx.await.unwrap_or(None);
        Ok(tun_index)
    }

    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        let inbound_rx = match self.inbound_rx.as_mut() {
            Some(rx) => rx,
            None => {
                tracing::error!("device handler recv none");
                return None;
            }
        };

        let result = inbound_rx.recv().await;
        if result.is_some() {
            self.rx_bytes += result.as_ref().unwrap().len();
        }
        result
    }

    pub async fn send(&mut self, packet: Vec<u8>) -> crate::Result<()> {
        let outbound_tx = match self.outbound_tx.as_ref() {
            Some(tx) => tx,
            None => {
                return Err("device handler send none".into());
            }
        };
        self.tx_bytes+=packet.len();
        tracing::debug!("device => server outbound tx len: {}", packet.len());
        let result = outbound_tx.send(packet).await;
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

impl Default for DeviceHandler {
    fn default() -> Self {
        Self::new()
    }
}