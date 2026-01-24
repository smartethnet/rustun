use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
#[allow(unused_imports)]
use tun::AbstractDevice;
use crate::codec::frame::{HandshakeReplyFrame, PeerDetail};
use crate::utils::sys_route::SysRoute;
use std::collections::{HashSet, HashMap};
#[allow(unused_imports)]
use crate::utils::sys_route::{ip_to_network, mask_to_prefix_length};

const DEFAULT_MTU: u16 = 1430;

#[derive(Clone)]
pub struct DeviceConfig {
    pub ip: String,
    pub mask: String,
    pub gateway: String,
    pub mtu: u16,
}

pub struct Device {
    ip: String,
    mask: String,
    mtu: u16,
    inbound_tx: mpsc::Sender<Vec<u8>>,
    outbound_rx: mpsc::Receiver<Vec<u8>>,
}

impl Device {
    pub fn new(
        ip: String,
        mask: String,
        mtu: u16,
        inbound_tx: mpsc::Sender<Vec<u8>>,
        outbound_rx: mpsc::Receiver<Vec<u8>>,
    ) -> Self {
        Self {
            ip,
            mask,
            mtu,
            inbound_tx,
            outbound_rx,
        }
    }

    pub async fn run(&mut self, ready: oneshot::Sender<Option<i32>>, name: oneshot::Sender<Option<String>>) -> crate::Result<()> {
        let mut config = tun::Configuration::default();
        config
            .address(self.ip.clone())
            .netmask(self.mask.clone())
            // .destination(self.config.gateway.clone())
            .mtu(self.mtu)
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

        // Get interface name (Linux only, for MASQUERADE)
        #[cfg(target_os = "linux")]
        {
            let interface_name = dev.tun_name().ok();
            let _ = name.send(interface_name);
        }
        
        #[cfg(not(target_os = "linux"))]
        let _ = name.send(None);

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
    peer_details: Vec<PeerDetail>,
    private_ip: String,
    mask: String,
    local_ciders: Vec<String>,
    tun_index: Option<i32>,
    interface_name: Option<String>,
    inbound_rx: Option<mpsc::Receiver<Vec<u8>>>,
    outbound_tx: Option<mpsc::Sender<Vec<u8>>>,
    pub rx_bytes: usize,
    pub tx_bytes: usize,
}

impl DeviceHandler {
    pub fn new() -> Self {
        Self {
            peer_details: vec![],
            private_ip: String::new(),
            mask: String::new(),
            local_ciders: vec![],
            tun_index: None,
            interface_name: None,
            inbound_rx: None,
            outbound_tx: None,
            rx_bytes: 0,
            tx_bytes: 0,
        }
    }

    pub async fn run(&mut self, cfg: &HandshakeReplyFrame, enable_masq: bool) -> crate::Result<Option<i32>> {
        let (inbound_tx, inbound_rx) = mpsc::channel(1000);
        let (outbound_tx, outbound_rx) = mpsc::channel(1000);
        self.inbound_rx = Some(inbound_rx);
        self.outbound_tx = Some(outbound_tx);
        self.private_ip = cfg.private_ip.clone();
        self.mask = cfg.mask.clone();
        self.local_ciders = cfg.ciders.clone();

        let mut dev = Device::new(cfg.private_ip.clone(),
                                  cfg.mask.clone(),
                                  DEFAULT_MTU,
                                  inbound_tx, outbound_rx);
        let (ready_tx, ready_rx) = oneshot::channel();
        let (name_tx, name_rx) = oneshot::channel();
        tokio::spawn(async move {
            let res = dev.run(ready_tx, name_tx).await;
            match res {
                Ok(_) => (),
                Err(e) => tracing::error!("device handler fail: {:?}", e),
            }
        });

        let tun_index = ready_rx.await.unwrap_or(None);
        self.tun_index = tun_index;

        if let Ok(Some(name)) = name_rx.await {
            self.interface_name = Some(name);
        }


        if enable_masq {
            if let Err(e) = self.enable_masquerade() {
                tracing::error!("Failed to enable MASQUERADE: {:?}", e);
            }
            if let Err(e) = self.enable_snat() {
                tracing::warn!("Failed to enable SNAT: {:?}", e);
            }
        }

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

    /// Get current peer details list
    pub fn get_peer_details(&self) -> Vec<PeerDetail> {
        self.peer_details.clone()
    }

    pub async fn reload_route(&mut self, new_routes: Vec<PeerDetail>) {
        let sys_route = SysRoute::new();
        
        let mut old_ciders: HashSet<String> = HashSet::new();
        for route in &self.peer_details {
            for cidr in &route.ciders {
                old_ciders.insert(cidr.clone());
            }
        }
        
        let mut new_ciders: HashSet<String> = HashSet::new();
        for route in &new_routes {
            for cidr in &route.ciders {
                new_ciders.insert(cidr.clone());
            }
        }
        
        tracing::info!("Reloading routes: old={}, new={}", old_ciders.len(), new_ciders.len());
        
        // Find routes to delete (in old but not in new)
        let to_delete: Vec<String> = old_ciders.difference(&new_ciders).cloned().collect();
        
        // Find routes to add (in new but not in old)
        let to_add: Vec<String> = new_ciders.difference(&old_ciders).cloned().collect();
        
        // Delete old routes
        for cidr in to_delete {
            tracing::info!("Deleting route: {}", cidr);
            if let Err(e) = sys_route.del(vec![cidr.clone()], self.private_ip.clone(), self.tun_index) {
                tracing::error!("Failed to delete route {}: {}", cidr, e);
            }
        }
        
        // Add new routes
        for cidr in to_add {
            tracing::info!("Adding route: {} via {}", cidr, self.private_ip);
            if let Err(e) = sys_route.add(vec![cidr.clone()], self.private_ip.clone(), self.tun_index) {
                tracing::error!("Failed to add route {}: {}", cidr, e);
            }
        }
        
        // Update stored routes
        self.peer_details = new_routes;

        tracing::info!("Route reload complete");
    }

    /// Enable MASQUERADE (NAT) for VPN interface (Linux only)
    /// Uses source network address instead of interface name for better reliability
    pub fn enable_masquerade(&mut self) -> crate::Result<()> {
        let cidr = self.ip_mask_to_cidr(&self.private_ip, &self.mask)?;
        
        let sys_route = SysRoute::new();
        sys_route.enable_masquerade_by_source(&cidr)?;
        Ok(())
    }

    /// Disable MASQUERADE (NAT) for VPN interface (Linux only)
    pub fn disable_masquerade(&mut self) -> crate::Result<()> {
        let cidr = self.ip_mask_to_cidr(&self.private_ip, &self.mask)?;
        
        let sys_route = SysRoute::new();
        sys_route.disable_masquerade_by_source(&cidr)?;
        Ok(())
    }

    /// Convert IP address and subnet mask to CIDR notation
    fn ip_mask_to_cidr(&self, ip: &str, mask: &str) -> crate::Result<String> {
        // Parse subnet mask to prefix length
        let prefix_len = mask_to_prefix_length(mask)?;
        let network = ip_to_network(ip, mask)?;
        Ok(format!("{}/{}", network, prefix_len))
    }

    /// Enable SNAT for local network segments to use virtual IP (Linux only)
    /// This makes packets from local ciders appear as coming from virtual IP
    pub fn enable_snat(&mut self) -> crate::Result<()> {
        let sys_route = SysRoute::new();
        
        for cidr in &self.local_ciders {
            sys_route.enable_snat_for_local_network(cidr, "", &self.private_ip)?;
            tracing::info!("Enabled SNAT for local network {} -> {}", cidr, self.private_ip);
        }
        Ok(())
    }

    /// Disable SNAT for local network segments (Linux only)
    pub fn disable_snat(&mut self) -> crate::Result<()> {
        let sys_route = SysRoute::new();
        
        for cidr in &self.local_ciders {
            sys_route.disable_snat_for_local_network(cidr, "", &self.private_ip)?;
        }
        Ok(())
    }

    /// Setup CIDR mapping DNAT rules based on HandshakeReplyFrame
    /// This should be called after receiving HandshakeReplyFrame
    /// Maps destination IPs from mapped CIDR to real CIDR using iptables NETMAP
    #[cfg(target_os = "linux")]
    pub fn setup_cidr_mapping(&mut self, cidr_mapping: &HashMap<String, String>) -> crate::Result<()> {
        let sys_route = SysRoute::new();
        
        for (mapped_cidr, real_cidr) in cidr_mapping {
            // Add DNAT rule (iptables will check if it already exists)
            if let Err(e) = sys_route.enable_cidr_dnat(mapped_cidr, real_cidr) {
                tracing::error!(
                    "Failed to add DNAT rule for {} -> {}: {}", 
                    mapped_cidr, real_cidr, e
                );
                return Err(e);
            }
            
            tracing::info!("Added DNAT rule for CIDR mapping: {} -> {}", mapped_cidr, real_cidr);
        }
        
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn setup_cidr_mapping(&mut self, _cidr_mapping: &HashMap<String, String>) -> crate::Result<()> {
        Ok(())
    }
}

impl Default for DeviceHandler {
    fn default() -> Self {
        Self::new()
    }
}