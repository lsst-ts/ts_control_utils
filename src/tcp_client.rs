// This file is part of ts_control_utils.
//
// Developed for the Vera Rubin Observatory Systems.
// This product includes software developed by the LSST Project
// (https://www.lsst.org).
// See the COPYRIGHT file at the top-level directory of this distribution
// for details of code ownership.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use log::info;
use std::net::TcpStream;

use crate::tcp_client_server_common::{TcpClientServerCommon, TcpConnection};

pub struct TcpClient {
    // Connection state.
    connection: TcpConnection,
    // Hostname or IP address of the server.
    pub host: String,
    // Port number of the server.
    pub port: i32,
}

impl TcpClient {
    /// Create a new TcpClient instance.
    ///
    /// # Arguments
    /// * `name` - Name of the client.
    /// * `host` - A string slice that holds the hostname or IP address.
    /// * `port` - An integer that holds the port number. Put 0 to let the OS
    ///   choose the port number.
    /// * `terminator` - A vector that holds the terminator.
    ///
    /// # Returns
    /// A TcpClient instance.
    pub fn new(name: &str, host: &str, port: i32, terminator: &[u8]) -> Self {
        Self {
            connection: TcpConnection::new(name, terminator),

            host: String::from(host),
            port,
        }
    }

    /// Connect to the server.
    ///
    /// # Returns
    /// A boolean value. If the connection is accepted, return true. Otherwise,
    /// return false.
    pub fn connect(&mut self) -> bool {
        let address = format!("{}:{}", self.host, self.port);
        if let Ok(stream) = TcpStream::connect(address) {
            if let Ok(peer_addr) = stream.peer_addr() {
                info!("{} is connected to {}.", self.connection.name, peer_addr);
            }

            self.attach_stream(stream);

            return true;
        }

        false
    }
}

impl TcpClientServerCommon for TcpClient {
    const PEER: &'static str = "server";

    fn connection(&self) -> &TcpConnection {
        &self.connection
    }

    fn connection_mut(&mut self) -> &mut TcpConnection {
        &mut self.connection
    }

    fn set_connected(&mut self, _is_connected: bool) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::{Value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::thread::{sleep, spawn};
    use std::time::Duration;

    use crate::constants::{LOCAL_HOST, TERMINATOR};
    use crate::tcp_server::TcpServer;

    const SLEEP_TIME: u64 = 50;
    const MAX_TIMEOUT: u64 = 300;

    fn create_tcp_server() -> (TcpServer, Arc<AtomicBool>) {
        let stop = Arc::new(AtomicBool::new(false));
        let server = TcpServer::new(
            "server",
            LOCAL_HOST,
            0,
            SLEEP_TIME,
            &TERMINATOR.to_vec(),
            &stop,
        );

        (server, stop)
    }

    fn create_tcp_client(port: i32) -> TcpClient {
        TcpClient::new("client", LOCAL_HOST, port, &TERMINATOR.to_vec())
    }

    fn callback_periodic_server(server: &mut TcpServer, _other: &mut i32) {
        let data = server.read_string();
        if !data.is_empty() {
            server.write_string(data, true);
        }

        sleep(Duration::from_millis(200));
    }

    fn accept_connection(client: &mut TcpClient) -> bool {
        let max_timeout_counter = MAX_TIMEOUT / SLEEP_TIME;

        let mut timeout_counter = 0;
        while timeout_counter < max_timeout_counter {
            if client.connect() {
                return true;
            } else {
                timeout_counter += 1;

                sleep(Duration::from_millis(SLEEP_TIME));
            }
        }

        false
    }

    fn wait_json_data_from_server(client: &mut TcpClient) -> Value {
        // wait_data_from_server(client, TcpClient::read_json, Value::is_null)

        let max_timeout_counter = MAX_TIMEOUT / SLEEP_TIME;

        let mut timeout_counter = 0;
        while timeout_counter < max_timeout_counter {
            let data = client.read_json();
            if !data.is_null() {
                return data;
            }

            timeout_counter += 1;
            sleep(Duration::from_millis(SLEEP_TIME));
        }

        Value::Null
    }

    #[test]
    fn test_connect_fail() {
        let mut client = create_tcp_client(1000);

        assert!(!client.connect());
    }

    #[test]
    fn test_connect_success() {
        let (mut server, stop) = create_tcp_server();
        let port = server.get_port();

        let handle = spawn(move || {
            server.run(
                callback_periodic_server,
                None::<fn(&mut TcpServer, &mut i32)>,
                &mut 0,
            );
        });

        let mut client = create_tcp_client(port);

        // Wait until the client connects to the server.
        assert!(accept_connection(&mut client));

        // close the server.
        stop.store(true, Ordering::Relaxed);

        assert!(handle.join().is_ok());
    }

    #[test]
    fn test_is_connected() {
        let (mut server, stop) = create_tcp_server();
        let port = server.get_port();

        let handle = spawn(move || {
            server.run(
                callback_periodic_server,
                None::<fn(&mut TcpServer, &mut i32)>,
                &mut 0,
            );
        });

        let mut client = create_tcp_client(port);

        // Not connected.
        assert!(!client.is_connected());

        // Connected.
        assert!(accept_connection(&mut client));
        assert!(client.is_connected());

        // close the server.
        stop.store(true, Ordering::Relaxed);

        assert!(handle.join().is_ok());
    }

    #[test]
    fn test_read_write_json() {
        let (mut server, stop) = create_tcp_server();
        let port = server.get_port();

        let handle = spawn(move || {
            server.run(
                callback_periodic_server,
                None::<fn(&mut TcpServer, &mut i32)>,
                &mut 0,
            );
        });

        let mut client = create_tcp_client(port);

        // Not connected. But it is safe to read the JSON data.
        assert!(client.read_json().is_null());

        // Connected.
        accept_connection(&mut client);
        assert!(client.is_connected());

        // Write the JSON data
        client.write_json(&json!({"id":"name","value":1}));

        assert_eq!(
            wait_json_data_from_server(&mut client),
            json!({"id":"name","value":1})
        );

        // Close the connection.
        stop.store(true, Ordering::Relaxed);

        assert!(handle.join().is_ok());
    }
}
