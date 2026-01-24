use std::process::Command;
use std::net::Ipv4Addr;
use ipnet::Ipv4Net;

pub struct SysRoute;

/// Convert subnet mask to prefix length
/// Example: "255.255.255.0" -> 24
pub(crate) fn mask_to_prefix_length(mask: &str) -> crate::Result<u8> {
    let mask_addr: Ipv4Addr = mask.parse()
        .map_err(|_| format!("Invalid subnet mask format: {}", mask))?;
    
    // Use ipnet library to convert mask to prefix length
    // We use UNSPECIFIED (0.0.0.0) as the base IP since we only care about the mask
    let net = Ipv4Net::with_netmask(Ipv4Addr::UNSPECIFIED, mask_addr)
        .map_err(|e| format!("Invalid subnet mask {}: {}", mask, e))?;
    
    Ok(net.prefix_len())
}

/// Convert IP address and subnet mask to network address
/// Example: ("10.0.0.1", "255.255.255.0") -> "10.0.0.0"
pub(crate) fn ip_to_network(ip: &str, mask: &str) -> crate::Result<String> {
    let ip_addr: Ipv4Addr = ip.parse()
        .map_err(|_| format!("Invalid IP address: {}", ip))?;
    let mask_addr: Ipv4Addr = mask.parse()
        .map_err(|_| format!("Invalid subnet mask: {}", mask))?;
    
    // Use ipnet library to get network address
    let net = Ipv4Net::with_netmask(ip_addr, mask_addr)
        .map_err(|e| format!("Invalid IP/mask combination ({} / {}): {}", ip, mask, e))?;
    
    Ok(net.network().to_string())
}

impl SysRoute {
    pub fn new() -> Self {
        Self
    }

    /// Check if iptables command is available (Linux only)
    /// This should be called before enabling MASQUERADE/SNAT features
    #[cfg(target_os = "linux")]
    pub fn check_iptables_available() -> crate::Result<()> {
        let output = Command::new("iptables")
            .args(["--version"])
            .output();
        
        match output {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(format!(
                    "iptables command not found. The --masq option requires iptables.\n\
                    Please either:\n\
                    1. Install iptables: sudo apt-get install iptables (Debian/Ubuntu) or sudo yum install iptables (RHEL/CentOS)\n\
                    2. Run without --masq option"
                ).into())
            }
            Err(e) => Err(format!("Failed to check iptables: {}", e).into()),
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn check_iptables_available() -> crate::Result<()> {
        Ok(())
    }

    /// Add routes to the system routing table
    /// - dsts: destination CIDR addresses (e.g., ["192.168.1.0/24", "10.0.0.0/8"])
    /// - gateway: gateway IP address
    /// - interface_idx: optional interface index (Windows only)
    pub fn add(&self, dsts: Vec<String>, gateway: String, interface_idx: Option<i32>) -> crate::Result<()> {
        for dst in dsts {
            self.add_route(&dst, &gateway, interface_idx)?
        }
        Ok(())
    }

    /// Delete routes from the system routing table
    /// - dsts: destination CIDR addresses
    /// - gateway: gateway IP address
    /// - interface_idx: optional interface index (Windows only)
    #[allow(unused)]
    pub fn del(&self, dsts: Vec<String>, gateway: String, interface_idx: Option<i32>) -> crate::Result<()> {
        for dst in dsts {
            self.del_route(&dst, &gateway, interface_idx)?
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn add_route(&self, dst: &str, gateway: &str, _interface_idx: Option<i32>) -> crate::Result<()> {
        let output = Command::new("ip")
            .args(["route", "add", dst, "via", gateway])
            .output()
            .map_err(|e| format!("Failed to execute ip command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to add route: {}", stderr).into());
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn del_route(&self, dst: &str, gateway: &str, _interface_idx: Option<i32>) -> crate::Result<()> {
        let output = Command::new("ip")
            .args(["route", "del", dst, "via", gateway])
            .output()
            .map_err(|e| format!("Failed to execute ip command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to delete route: {}", stderr).into());
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn add_route(&self, dst: &str, gateway: &str, _interface_idx: Option<i32>) -> crate::Result<()> {
        let output = Command::new("route")
            .args(["-n", "add", "-net", dst, gateway])
            .output()
            .map_err(|e| format!("Failed to execute route command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to add route: {}", stderr).into());
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn del_route(&self, dst: &str, gateway: &str, _interface_idx: Option<i32>) -> crate::Result<()> {
        let output = Command::new("route")
            .args(["-n", "delete", "-net", dst, gateway])
            .output()
            .map_err(|e| format!("Failed to execute route command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to delete route: {}", stderr).into());
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn add_route(&self, dst: &str, gateway: &str, interface_idx: Option<i32>) -> crate::Result<()> {
        // Windows route command format: route add <network> mask <netmask> <gateway> if <interface_idx> metric 1
        let (network, mask) = self.parse_cidr(dst)?;

        let mut args = vec!["add", &network, "mask", &mask, gateway];
        
        // Add interface index if provided
        let idx_str;
        if let Some(idx) = interface_idx {
            idx_str = idx.to_string();
            args.push("if");
            args.push(&idx_str);
        }
        
        // Always use metric 1 for highest priority
        args.push("metric");
        args.push("1");

        let output = Command::new("route")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to execute route command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Ignore "already exists" error
            if stderr.contains("already exists") || stderr.contains("已存在") {
                tracing::debug!("Route already exists: {} via {}", dst, gateway);
                return Ok(());
            }
            return Err(format!("Failed to add route: {}", stderr).into());
        }
        
        tracing::debug!("Added route: {} via {} (interface: {:?})", dst, gateway, interface_idx);
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn del_route(&self, dst: &str, _gateway: &str, _interface_idx: Option<i32>) -> crate::Result<()> {
        let (network, mask) = self.parse_cidr(dst)?;

        let output = Command::new("route")
            .args(&["delete", &network, "mask", &mask])
            .output()
            .map_err(|e| format!("Failed to execute route command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Ignore "not found" error
            if stderr.contains("not found") || stderr.contains("找不到") {
                tracing::debug!("Route not found (already deleted): {}", dst);
                return Ok(());
            }
            return Err(format!("Failed to delete route: {}", stderr).into());
        }
        Ok(())
    }

    #[allow(unused)]
    fn parse_cidr(&self, cidr: &str) -> crate::Result<(String, String)> {
        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid CIDR format: {}", cidr).into());
        }

        let network = parts[0].to_string();
        let prefix_len: u8 = parts[1]
            .parse()
            .map_err(|_| format!("Invalid prefix length: {}", parts[1]))?;

        // Convert prefix length to netmask
        let mask = Self::prefix_to_netmask(prefix_len)?;
        Ok((network, mask))
    }

    fn prefix_to_netmask(prefix_len: u8) -> crate::Result<String> {
        if prefix_len > 32 {
            return Err("Invalid prefix length: must be 0-32".into());
        }

        let mask_int = (!0u32) << (32 - prefix_len);
        let octets = [
            ((mask_int >> 24) & 0xFF) as u8,
            ((mask_int >> 16) & 0xFF) as u8,
            ((mask_int >> 8) & 0xFF) as u8,
            (mask_int & 0xFF) as u8,
        ];

        Ok(format!(
            "{}.{}.{}.{}",
            octets[0], octets[1], octets[2], octets[3]
        ))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    fn add_route(&self, _dst: &str, _gateway: &str) -> crate::Result<()> {
        Err("Route management is not supported on this platform".into())
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    fn del_route(&self, _dst: &str, _gateway: &str) -> crate::Result<()> {
        Err("Route management is not supported on this platform".into())
    }

    /// Enable MASQUERADE (NAT) for VPN interface using source network address (Linux only)
    /// This allows VPN clients to access external networks through the VPN gateway
    /// Uses source network CIDR instead of interface name for better reliability
    #[cfg(target_os = "linux")]
    pub fn enable_masquerade_by_source(&self, source_cidr: &str) -> crate::Result<()> {
        // Check if rule already exists: iptables -t nat -C POSTROUTING -s <source_cidr> -j MASQUERADE
        let check_output = Command::new("iptables")
            .args([
                "-t", "nat",
                "-C", "POSTROUTING",
                "-s", source_cidr,
                "-j", "MASQUERADE"
            ])
            .output()
            .map_err(|e| format!("Failed to execute iptables check command: {}", e))?;

        if check_output.status.success() {
            tracing::debug!("MASQUERADE rule already exists for source {}", source_cidr);
            return Ok(());
        }

        // Add iptables rule: iptables -t nat -A POSTROUTING -s <source_cidr> -j MASQUERADE
        let output = Command::new("iptables")
            .args([
                "-t", "nat",
                "-A", "POSTROUTING",
                "-s", source_cidr,
                "-j", "MASQUERADE"
            ])
            .output()
            .map_err(|e| format!("Failed to execute iptables command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to enable MASQUERADE: {}", stderr).into());
        }

        tracing::info!("Enabled MASQUERADE for source network: {}", source_cidr);
        Ok(())
    }

    /// Disable MASQUERADE (NAT) for VPN interface using source network address (Linux only)
    #[cfg(target_os = "linux")]
    pub fn disable_masquerade_by_source(&self, source_cidr: &str) -> crate::Result<()> {
        // Remove iptables rule: iptables -t nat -D POSTROUTING -s <source_cidr> -j MASQUERADE
        let output = Command::new("iptables")
            .args([
                "-t", "nat",
                "-D", "POSTROUTING",
                "-s", source_cidr,
                "-j", "MASQUERADE"
            ])
            .output()
            .map_err(|e| format!("Failed to execute iptables command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to disable MASQUERADE: {}", stderr).into());
        }

        tracing::info!("Disabled MASQUERADE for source network: {}", source_cidr);
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn enable_masquerade_by_source(&self, _interface: &str) -> crate::Result<()> {
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn disable_masquerade_by_source(&self, _interface: &str) -> crate::Result<()> {
        Ok(())
    }

    /// Enable SNAT for local network segments to use virtual IP (Linux only)
    /// This allows packets from local ciders to appear as coming from virtual IP
    /// Rule: iptables -t nat -A POSTROUTING -s <local_cidr> -j SNAT --to-source <virtual_ip>
    #[cfg(target_os = "linux")]
    pub fn enable_snat_for_local_network(&self, local_cidr: &str, _tun_interface: &str, virtual_ip: &str) -> crate::Result<()> {
        // Check if rule already exists: iptables -t nat -C POSTROUTING -s <local_cidr> -j SNAT --to-source <virtual_ip>
        let check_output = Command::new("iptables")
            .args([
                "-t", "nat",
                "-C", "POSTROUTING",
                "-s", local_cidr,
                "-j", "SNAT",
                "--to-source", virtual_ip
            ])
            .output()
            .map_err(|e| format!("Failed to execute iptables check command: {}", e))?;

        if check_output.status.success() {
            tracing::debug!("SNAT rule already exists for {} -> {}", local_cidr, virtual_ip);
            return Ok(());
        }

        // Add iptables rule: iptables -t nat -A POSTROUTING -s <local_cidr> -j SNAT --to-source <virtual_ip>
        let output = Command::new("iptables")
            .args([
                "-t", "nat",
                "-A", "POSTROUTING",
                "-s", local_cidr,
                "-j", "SNAT",
                "--to-source", virtual_ip
            ])
            .output()
            .map_err(|e| format!("Failed to execute iptables command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to enable SNAT: {}", stderr).into());
        }

        tracing::info!("Enabled SNAT for {} -> {}", local_cidr, virtual_ip);
        Ok(())
    }

    /// Disable SNAT for local network segments (Linux only)
    #[cfg(target_os = "linux")]
    pub fn disable_snat_for_local_network(&self, local_cidr: &str, _tun_interface: &str, virtual_ip: &str) -> crate::Result<()> {
        let output = Command::new("iptables")
            .args([
                "-t", "nat",
                "-D", "POSTROUTING",
                "-s", local_cidr,
                "-j", "SNAT",
                "--to-source", virtual_ip
            ])
            .output()
            .map_err(|e| format!("Failed to execute iptables command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to disable SNAT: {}", stderr).into());
        }

        tracing::info!("Disabled SNAT for {} -> {}", local_cidr, virtual_ip);
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn enable_snat_for_local_network(&self, _local_cidr: &str, _tun_interface: &str, _virtual_ip: &str) -> crate::Result<()> {
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn disable_snat_for_local_network(&self, _local_cidr: &str, _tun_interface: &str, _virtual_ip: &str) -> crate::Result<()> {
        Ok(())
    }

    /// Enable DNAT/NETMAP for CIDR mapping (Linux only)
    /// Maps destination IPs from mapped CIDR to real CIDR
    /// Uses NETMAP target: iptables -t nat -A PREROUTING -d <mapped_cidr> -j NETMAP --to <real_cidr>
    /// 
    /// # Arguments
    /// * `mapped_cidr` - The CIDR that other clients see (e.g., "192.168.11.0/24")
    /// * `real_cidr` - The real CIDR network (e.g., "192.168.10.0/24")
    /// 
    /// # Example
    /// When a packet arrives with destination IP in `mapped_cidr`, it will be translated
    /// to the corresponding IP in `real_cidr` before being forwarded to the local network.
    #[cfg(target_os = "linux")]
    pub fn enable_cidr_dnat(&self, mapped_cidr: &str, real_cidr: &str) -> crate::Result<()> {
        // Check if NETMAP rule already exists: iptables -t nat -C PREROUTING -d <mapped_cidr> -j NETMAP --to <real_cidr>
        let check_output = Command::new("iptables")
            .args([
                "-t", "nat",
                "-C", "PREROUTING",
                "-d", mapped_cidr,
                "-j", "NETMAP",
                "--to", real_cidr,
            ])
            .output();

        match check_output {
            Ok(output) if output.status.success() => {
                tracing::debug!("DNAT rule already exists: {} -> {}", mapped_cidr, real_cidr);
                return Ok(());
            }
            _ => {}
        }

        // Add NETMAP rule: iptables -t nat -A PREROUTING -d <mapped_cidr> -j NETMAP --to <real_cidr>
        let output = Command::new("iptables")
            .args([
                "-t", "nat",
                "-A", "PREROUTING",
                "-d", mapped_cidr,
                "-j", "NETMAP",
                "--to", real_cidr,
            ])
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    format!(
                        "iptables command not found. CIDR mapping requires iptables with NETMAP support.\n\
                        Please install iptables and ensure your kernel supports NETMAP target.\n\
                        NETMAP requires Linux kernel 2.6.32+ with netfilter NETMAP module."
                    )
                } else {
                    format!("Failed to execute iptables command: {}", e)
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Check if NETMAP is not supported
            if stderr.contains("No chain/target/match") || stderr.contains("NETMAP") {
                return Err(format!(
                    "NETMAP target not supported. CIDR mapping requires kernel support for NETMAP.\n\
                    Please ensure your kernel has NETMAP support (Linux 2.6.32+) or use a different approach.\n\
                    Error: {}", stderr
                ).into());
            }
            return Err(format!("Failed to add DNAT rule: {}", stderr).into());
        }

        tracing::info!("Added DNAT rule: {} -> {}", mapped_cidr, real_cidr);
        Ok(())
    }

    /// Disable DNAT/NETMAP for CIDR mapping (Linux only)
    /// Removes the NETMAP rule that was previously added
    /// 
    /// # Arguments
    /// * `mapped_cidr` - The mapped CIDR (e.g., "192.168.11.0/24")
    /// * `real_cidr` - The real CIDR (e.g., "192.168.10.0/24")
    #[cfg(target_os = "linux")]
    pub fn disable_cidr_dnat(&self, mapped_cidr: &str, real_cidr: &str) -> crate::Result<()> {
        let output = Command::new("iptables")
            .args([
                "-t", "nat",
                "-D", "PREROUTING",
                "-d", mapped_cidr,
                "-j", "NETMAP",
                "--to", real_cidr,
            ])
            .output()
            .map_err(|e| format!("Failed to execute iptables command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Ignore "not found" error (rule already deleted)
            if stderr.contains("not found") || stderr.contains("找不到") || stderr.contains("No rule") {
                tracing::debug!("DNAT rule not found (already deleted): {} -> {}", mapped_cidr, real_cidr);
                return Ok(());
            }
            return Err(format!("Failed to delete DNAT rule: {}", stderr).into());
        }

        tracing::info!("Deleted DNAT rule: {} -> {}", mapped_cidr, real_cidr);
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn enable_cidr_dnat(&self, _mapped_cidr: &str, _real_cidr: &str) -> crate::Result<()> {
        Err("CIDR mapping DNAT is only supported on Linux".into())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn disable_cidr_dnat(&self, _mapped_cidr: &str, _real_cidr: &str) -> crate::Result<()> {
        Err("CIDR mapping DNAT is only supported on Linux".into())
    }
}

impl Default for SysRoute {
    fn default() -> Self {
        Self::new()
    }
}
