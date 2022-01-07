use std::{
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpStream},
    str,
    str::FromStr,
};

use crate::{
    models::{AdbCommand, AdbRequestStatus, AdbVersion, DeviceLong},
    AdbCommandProvider, Device, Result, RustADBError,
};

/// Represents an ADB-over-TCP connexion.
pub struct AdbTcpConnexion {
    socket_addr: SocketAddrV4,
    port: u16,
    address: Ipv4Addr,
}

impl AdbTcpConnexion {
    /// Instantiates a new instance of [AdbTcpConnexion]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a custom listening port for ADB server.
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self.socket_addr = SocketAddrV4::new(self.address, self.port);
        self
    }

    /// Sets a custom listening address for ADB server.
    pub fn address(mut self, address: Ipv4Addr) -> Self {
        self.address = address;
        self.socket_addr = SocketAddrV4::new(self.address, self.port);
        self
    }

    fn proxy_connexion(&self, adb_command: AdbCommand, with_response: bool) -> Result<Vec<u8>> {
        let mut tcp_stream = TcpStream::connect(self.socket_addr)?;

        Self::send_adb_request(&mut tcp_stream, adb_command)?;

        let mut request_status = [0; 4];
        tcp_stream.read_exact(&mut request_status)?;

        match AdbRequestStatus::from_str(str::from_utf8(&request_status.to_vec())?)? {
            AdbRequestStatus::Okay if with_response => {
                let length = Self::get_body_length(&mut tcp_stream)?;

                let mut body = vec![
                    0;
                    length
                        .try_into()
                        .map_err(|_| RustADBError::ConvertionError)?
                ];
                if length > 0 {
                    tcp_stream.read_exact(&mut body)?;
                }

                Ok(body)
            }
            AdbRequestStatus::Okay => Ok(vec![]),
            AdbRequestStatus::Fail => {
                let length = Self::get_body_length(&mut tcp_stream)?;

                let mut body = vec![
                    0;
                    length
                        .try_into()
                        .map_err(|_| RustADBError::ConvertionError)?
                ];
                if length > 0 {
                    tcp_stream.read_exact(&mut body)?;
                }

                Err(RustADBError::ADBRequestFailed(String::from_utf8(body)?))
            }
        }
    }

    fn send_adb_request(tcp_stream: &mut TcpStream, command: AdbCommand) -> Result<()> {
        let adb_command_string = command.to_string();

        let adb_request = format!(
            "{}{}",
            format!("{:04x}", adb_command_string.len()),
            adb_command_string
        );

        tcp_stream.write_all(adb_request.as_bytes())?;

        Ok(())
    }

    fn get_body_length(tcp_stream: &mut TcpStream) -> Result<u32> {
        let mut length = [0; 4];
        tcp_stream.read_exact(&mut length)?;

        Ok(u32::from_str_radix(str::from_utf8(&length)?, 16)?)
    }
}

impl Default for AdbTcpConnexion {
    fn default() -> Self {
        let default_port: u16 = 5037;
        let default_address = Ipv4Addr::new(127, 0, 0, 1);

        Self {
            socket_addr: SocketAddrV4::new(default_address, default_port),
            address: default_address,
            port: default_port,
        }
    }
}

impl AdbCommandProvider for AdbTcpConnexion {
    fn version(&self) -> Result<AdbVersion> {
        let version = self.proxy_connexion(AdbCommand::Version, true)?;

        AdbVersion::try_from(version)
    }

    fn devices(&self) -> Result<Vec<Device>> {
        let devices = self.proxy_connexion(AdbCommand::Devices, true)?;

        let mut vec_devices: Vec<Device> = vec![];
        for device in devices.split(|x| x.eq(&b'\n')) {
            if device.is_empty() {
                break;
            }

            vec_devices.push(Device::try_from(device.to_vec())?);
        }

        Ok(vec_devices)
    }

    fn devices_long(&self) -> Result<Vec<DeviceLong>> {
        let devices_long = self.proxy_connexion(AdbCommand::DevicesLong, true)?;

        let mut vec_devices: Vec<DeviceLong> = vec![];
        for device in devices_long.split(|x| x.eq(&b'\n')) {
            if device.is_empty() {
                break;
            }

            vec_devices.push(DeviceLong::try_from(device.to_vec())?);
        }

        Ok(vec_devices)
    }

    fn kill(&self) -> Result<()> {
        self.proxy_connexion(AdbCommand::Kill, false).map(|_| ())
    }

    // TODO: Change with Generator when feature stabilizes
    fn track_devices(&self, callback: fn(Device) -> Result<()>) -> Result<()> {
        let mut tcp_stream = TcpStream::connect(self.socket_addr)?;

        Self::send_adb_request(&mut tcp_stream, AdbCommand::TrackDevices)?;

        let mut request_status = [0; 4];
        tcp_stream.read_exact(&mut request_status)?;

        match AdbRequestStatus::from_str(str::from_utf8(&request_status.to_vec())?)? {
            AdbRequestStatus::Okay => loop {
                let length = Self::get_body_length(&mut tcp_stream)?;

                if length > 0 {
                    let mut body = vec![
                        0;
                        length
                            .try_into()
                            .map_err(|_| RustADBError::ConvertionError)?
                    ];
                    tcp_stream.read_exact(&mut body)?;

                    callback(Device::try_from(body)?)?;
                }
            },
            AdbRequestStatus::Fail => {
                let length = Self::get_body_length(&mut tcp_stream)?;

                let mut body = vec![
                    0;
                    length
                        .try_into()
                        .map_err(|_| RustADBError::ConvertionError)?
                ];
                if length > 0 {
                    tcp_stream.read_exact(&mut body)?;
                }

                Err(RustADBError::ADBRequestFailed(String::from_utf8(body)?))
            }
        }
    }
}