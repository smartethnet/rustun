use std::process::Command;

pub struct SysRoute;

impl SysRoute {
    pub fn new() -> Self {
        Self
    }

    /// Add routes to the system routing table
    /// - dsts: destination CIDR addresses (e.g., ["192.168.1.0/24", "10.0.0.0/8"])
    /// - gateway: gateway IP address
    pub fn add(&self, dsts: Vec<String>, gateway: String) -> crate::Result<()> {
        for dst in dsts {
            self.add_route(&dst, &gateway)?
        }
        Ok(())
    }

    /// Delete routes from the system routing table
    /// - dsts: destination CIDR addresses
    /// - gateway: gateway IP address
    #[allow(unused)]
    pub fn del(&self, dsts: Vec<String>, gateway: String) -> crate::Result<()> {
        for dst in dsts {
            self.del_route(&dst, &gateway)?
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn add_route(&self, dst: &str, gateway: &str) -> crate::Result<()> {
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
    fn del_route(&self, dst: &str, gateway: &str) -> crate::Result<()> {
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
    fn add_route(&self, dst: &str, gateway: &str) -> crate::Result<()> {
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
    fn del_route(&self, dst: &str, gateway: &str) -> crate::Result<()> {
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
    fn add_route(&self, dst: &str, gateway: &str) -> crate::Result<()> {
        // Windows route command format: route add <network> mask <netmask> <gateway>
        let (network, mask) = self.parse_cidr(dst)?;

        let output = Command::new("route")
            .args(&["add", &network, "mask", &mask, gateway])
            .output()
            .map_err(|e| format!("Failed to execute route command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to add route: {}", stderr).into());
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn del_route(&self, dst: &str, gateway: &str) -> crate::Result<()> {
        let (network, mask) = self.parse_cidr(dst)?;

        let output = Command::new("route")
            .args(&["delete", &network, "mask", &mask, gateway])
            .output()
            .map_err(|e| format!("Failed to execute route command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to delete route: {}", stderr).into());
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
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

    #[cfg(target_os = "windows")]
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
}

impl Default for SysRoute {
    fn default() -> Self {
        Self::new()
    }
}
