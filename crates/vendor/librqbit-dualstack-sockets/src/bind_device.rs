#[cfg(test)]
pub(crate) mod tests;

use crate::Error;
use std::{ffi::CString, num::NonZeroU32, str::FromStr};

#[derive(Debug, Clone)]
pub struct BindDevice {
    #[allow(unused)]
    index: NonZeroU32,
    #[allow(unused)]
    name: CString,
}

impl BindDevice {
    #[cfg(not(windows))]
    pub fn new_from_name(name: &str) -> crate::Result<Self> {
        let name = CString::new(name).map_err(|_| Error::BindDeviceInvalid)?;

        let index = unsafe { libc::if_nametoindex(name.as_ptr()) };
        let index = NonZeroU32::new(index)
            .ok_or_else(|| Error::BindDeviceInvalidError(std::io::Error::last_os_error()))?;
        Ok(Self { index, name })
    }

    /// sin-e-phile: Windows has no SO_BINDTODEVICE, but it names adapters by their
    /// friendly name ("Ethernet", "Wi-Fi", a VPN's adapter), which `network-interface`
    /// reports along with the interface index the socket options below take.
    #[cfg(windows)]
    pub fn new_from_name(name: &str) -> crate::Result<Self> {
        use network_interface::{NetworkInterface, NetworkInterfaceConfig};
        let nics = NetworkInterface::show()
            .map_err(|e| Error::BindDeviceInvalidError(std::io::Error::other(e.to_string())))?;
        let nic = nics
            .into_iter()
            .find(|n| n.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                Error::BindDeviceInvalidError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("no network adapter called {name:?}"),
                ))
            })?;
        let index = NonZeroU32::new(nic.index).ok_or(Error::BindDeviceInvalid)?;
        let name = CString::new(name).map_err(|_| Error::BindDeviceInvalid)?;
        Ok(Self { index, name })
    }

    pub fn index(&self) -> NonZeroU32 {
        self.index
    }

    pub fn name(&self) -> &str {
        // We constructed from a string so this can't fail
        unsafe { std::str::from_utf8_unchecked(self.name.to_bytes()) }
    }

    #[cfg(target_os = "macos")]
    pub fn bind_sref(&self, sref: &socket2::Socket, is_v6: bool) -> crate::Result<()> {
        if is_v6 {
            sref.bind_device_by_index_v6(Some(self.index))
                .map_err(Error::BindDeviceSetDeviceError)
        } else {
            sref.bind_device_by_index_v4(Some(self.index))
                .map_err(Error::BindDeviceSetDeviceError)
        }
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    pub fn bind_sref(&self, sref: &socket2::Socket, _is_v6: bool) -> crate::Result<()> {
        let name = self.name.as_bytes_with_nul();
        sref.bind_device(Some(name))
            .map_err(Error::BindDeviceSetDeviceError)
    }

    /// sin-e-phile: `IP_UNICAST_IF` / `IPV6_UNICAST_IF` pin a socket's outgoing traffic
    /// to one interface. If that interface goes away — a VPN dropping — sends fail
    /// rather than leaving by another route, which is the point (SPEC.md Phase 7).
    /// IPv4 takes the index in network byte order; IPv6 in host order. A dual-stack
    /// IPv6 socket also carries IPv4-mapped traffic, so it gets both.
    #[cfg(windows)]
    pub fn bind_sref(&self, sref: &socket2::Socket, is_v6: bool) -> crate::Result<()> {
        use std::os::windows::io::AsRawSocket;
        use windows_sys::Win32::Networking::WinSock::{
            setsockopt, IPPROTO_IP, IPPROTO_IPV6, IPV6_UNICAST_IF, IP_UNICAST_IF, SOCKET_ERROR,
        };
        let raw = sref.as_raw_socket() as usize;
        let set = |level: i32, option: i32, value: u32| -> std::io::Result<()> {
            // SAFETY: `raw` is a live socket owned by `sref`, and the option value is a
            // 4-byte DWORD, which is what both options document.
            let rc = unsafe {
                setsockopt(
                    raw,
                    level,
                    option,
                    &value as *const u32 as *const u8,
                    std::mem::size_of::<u32>() as i32,
                )
            };
            if rc == SOCKET_ERROR {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        };
        let index = self.index.get();
        if is_v6 {
            set(IPPROTO_IPV6, IPV6_UNICAST_IF, index).map_err(Error::BindDeviceSetDeviceError)?;
            // Best effort: a v6-only socket refuses the v4 option, which is fine.
            let _ = set(IPPROTO_IP, IP_UNICAST_IF, index.to_be());
            Ok(())
        } else {
            set(IPPROTO_IP, IP_UNICAST_IF, index.to_be()).map_err(Error::BindDeviceSetDeviceError)
        }
    }
}

impl FromStr for BindDevice {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new_from_name(s)
    }
}
