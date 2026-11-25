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

use log::{debug, info};
use serde_json::{Value, from_str};
use std::io::{BufReader, BufWriter, Read, Write};
use std::net::{Shutdown, TcpStream};

const DEFAULT_BUFFER_SIZE: usize = 100;

/// State of a TCP/IP connection shared by the TCP client and server.
pub struct TcpConnection {
    // Name of the client or server.
    pub name: String,
    // Reader and writer for the TCP/IP connection.
    reader: Option<BufReader<TcpStream>>,
    writer: Option<BufWriter<TcpStream>>,
    // Buffer to read the received message.
    buffer: Vec<u8>,
    // Terminator of the message.
    terminator: Vec<u8>,
}

impl TcpConnection {
    /// Create a new TcpConnection instance.
    ///
    /// # Arguments
    /// * `name` - Name of the client or server.
    /// * `terminator` - A slice that holds the terminator.
    ///
    /// # Returns
    /// A TcpConnection instance.
    pub fn new(name: &str, terminator: &[u8]) -> Self {
        Self {
            name: String::from(name),
            reader: None,
            writer: None,
            buffer: Vec::with_capacity(DEFAULT_BUFFER_SIZE),
            terminator: terminator.to_owned(),
        }
    }

    /// Check if the reader and writer are available.
    ///
    /// # Returns
    /// True if both the reader and writer are available. Otherwise, false.
    pub fn has_stream(&self) -> bool {
        self.reader.is_some() && self.writer.is_some()
    }
}

/// Common functionalities of the TCP client and server. The implementer only
/// needs to provide the connection state, the connection flag, and the label
/// of the peer. The reading, writing, and closing are shared.
///
/// # Notes
/// The trait needs to be in scope to call its methods.
pub trait TcpClientServerCommon {
    /// Label of the peer in the log messages, e.g. "server" or "client".
    const PEER: &'static str;

    /// Get the connection state.
    fn connection(&self) -> &TcpConnection;

    /// Get the mutable connection state.
    fn connection_mut(&mut self) -> &mut TcpConnection;

    /// Update the connection flag.
    ///
    /// # Arguments
    /// * `is_connected` - Whether the connection is established.
    fn set_connected(&mut self, is_connected: bool);

    /// Check if the server is connected.
    ///
    /// # Returns
    /// A boolean value. If the server is connected, return true. Otherwise,
    /// return false.
    fn is_connected(&self) -> bool {
        self.connection().has_stream()
    }

    /// Attach the connected stream. This sets up the stream, wraps it with
    /// the reader and writer, and updates the connection flag.
    ///
    /// # Arguments
    /// * `stream` - The connected TCP stream.
    fn attach_stream(&mut self, stream: TcpStream) {
        stream
            .set_nodelay(true)
            .expect("Tcp stream should set nodelay.");
        stream
            .set_nonblocking(true)
            .expect("Tcp stream should set non-blocking.");

        self.set_connected(true);

        // Wrap the stream to the BufReader and BufWriter.
        let connection = self.connection_mut();
        connection.reader = Some(BufReader::new(
            stream.try_clone().expect("Tcp stream should clone."),
        ));
        connection.writer = Some(BufWriter::new(stream));
    }

    /// Read the JSON data.
    ///
    /// # Returns
    /// A Value instance that holds the JSON data.
    fn read_json(&mut self) -> Value {
        let data = self.read_string();
        match from_str(&data) {
            Ok(value) => value,
            Err(_) => {
                if !data.is_empty() {
                    info!(
                        "{} receives non-JSON message: {}.",
                        self.connection().name,
                        data
                    );
                }
                Value::Null
            }
        }
    }

    /// Read the string data.
    ///
    /// # Returns
    /// A string that holds the data.
    fn read_string(&mut self) -> String {
        let connection = self.connection_mut();
        let terminator_length = connection.terminator.len();
        // Always clear the buffer before reading the data.
        connection.buffer.clear();

        let mut is_disconnected = false;
        if let Some(stream) = connection.reader.as_mut() {
            let mut byte = [0; 1];
            loop {
                match stream.read(&mut byte) {
                    Ok(0) => {
                        // The peer is disconnected.
                        debug!(
                            "{} is disconnected from the {} when reading.",
                            connection.name,
                            Self::PEER
                        );
                        is_disconnected = true;
                        break;
                    }

                    Ok(_) => {
                        connection.buffer.push(byte[0]);
                        // Check the terminator.
                        if connection.buffer.ends_with(&connection.terminator) {
                            break;
                        }
                    }

                    Err(_) => {
                        break;
                    }
                }
            }
        }

        let message = if connection.buffer.len() < terminator_length {
            String::new()
        } else {
            match String::from_utf8(
                connection.buffer[..(connection.buffer.len() - terminator_length)].to_vec(),
            ) {
                Ok(message) => {
                    debug!("{} receives: {}.", connection.name, message);

                    message
                }
                Err(_) => String::new(),
            }
        };

        if is_disconnected {
            self.close_stream();
        }

        message
    }

    /// Write the JSON item.
    ///
    /// # Arguments
    /// * `item` - A Value instance that holds the JSON data.
    fn write_json(&mut self, item: &Value) {
        self.write_string(item.to_string(), true);
    }

    /// Write the JSON items. This is used for the telemetry. If there is still
    /// data in the buffer, we don't want to write the new data but to flush
    /// the existing data instead.
    ///
    /// # Arguments
    /// * `items` - A slice of Value instances that holds the JSON data.
    fn write_jsons(&mut self, items: &[Value]) {
        if let Some(stream) = self.connection_mut().writer.as_mut()
            && !stream.buffer().is_empty()
        {
            self.flush();
            return;
        }

        for item in items {
            self.write_string(item.to_string(), false);
        }
        self.flush();
    }

    /// Write the string data to buffer.
    ///
    /// # Notes
    /// Compared with the `self.write_json()` and `self.write_jsons()`, this is
    /// a low-level function call, and you need to decide whether to flush the
    /// stream after writing.
    ///
    /// # Arguments
    /// * `data` - A string that holds the data.
    /// * `flush` - Whether to flush the stream after writing the data.
    fn write_string(&mut self, data: String, flush: bool) {
        let connection = self.connection_mut();
        if let Some(stream) = connection.writer.as_mut() {
            // Add the terminator.
            let mut data_with_terminator = data;
            data_with_terminator.push_str(&String::from_utf8_lossy(&connection.terminator));

            // Write the data to the buffer. Ignore the possible error.
            let _ = stream.write(data_with_terminator.as_bytes());
        }

        if flush {
            self.flush();
        }
    }

    /// Flush the stream.
    fn flush(&mut self) {
        let connection = self.connection_mut();
        if let Some(stream) = connection.writer.as_mut()
            && let Err(error) = stream.flush()
        {
            match error.kind() {
                std::io::ErrorKind::WouldBlock => {
                    // The operation would block, we can try again later.
                }
                _ => {
                    // The peer is disconnected.
                    debug!(
                        "{} fails to flush the stream: {}. Diconnecting...",
                        connection.name, error
                    );
                    self.close_stream();
                }
            }
        }
    }

    /// Close the stream.
    fn close_stream(&mut self) {
        let connection = self.connection_mut();
        info!("{} is disconnected.", connection.name);

        // Flush the stream in the writer first before closing the stream.
        if let Some(stream) = connection.writer.as_mut() {
            // Ignore the possible error.
            let _ = stream.flush();
        }

        // Shutdown the stream in the reader and writer.
        if let Some(stream) = connection.reader.as_ref() {
            // Ignore the possible error.
            let _ = stream.get_ref().shutdown(Shutdown::Both);
        }

        connection.reader = None;
        connection.writer = None;

        self.set_connected(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;
    use std::net::TcpListener;
    use std::thread::sleep;
    use std::time::Duration;

    use crate::constants::{LOCAL_HOST, TERMINATOR};

    const SLEEP_TIME: u64 = 50;
    const MAX_RETRY: usize = 10;
    const READ_TIMEOUT: u64 = 2000;

    struct TestEndpoint {
        connection: TcpConnection,
        is_connected: bool,
    }

    impl TcpClientServerCommon for TestEndpoint {
        const PEER: &'static str = "peer";

        fn connection(&self) -> &TcpConnection {
            &self.connection
        }

        fn connection_mut(&mut self) -> &mut TcpConnection {
            &mut self.connection
        }

        fn set_connected(&mut self, is_connected: bool) {
            self.is_connected = is_connected;
        }
    }

    // Create a connected endpoint and the raw stream of its peer.
    fn create_endpoint_and_peer() -> (TestEndpoint, TcpStream) {
        let listener = TcpListener::bind(format!("{}:0", LOCAL_HOST)).unwrap();
        let port = listener.local_addr().unwrap().port();

        let peer = TcpStream::connect(format!("{}:{}", LOCAL_HOST, port)).unwrap();
        let (stream, _) = listener.accept().unwrap();

        let mut endpoint = TestEndpoint {
            connection: TcpConnection::new("endpoint", TERMINATOR),
            is_connected: false,
        };
        endpoint.attach_stream(stream);

        (endpoint, peer)
    }

    fn wait_string(endpoint: &mut TestEndpoint) -> String {
        wait_data(endpoint, TestEndpoint::read_string, String::is_empty)
    }

    fn wait_json(endpoint: &mut TestEndpoint) -> Value {
        wait_data(endpoint, TestEndpoint::read_json, Value::is_null)
    }

    fn wait_data<T: Default>(
        endpoint: &mut TestEndpoint,
        read: fn(&mut TestEndpoint) -> T,
        is_empty: fn(&T) -> bool,
    ) -> T {
        for _ in 0..MAX_RETRY {
            let data = read(endpoint);
            if !is_empty(&data) {
                return data;
            }

            sleep(Duration::from_millis(SLEEP_TIME));
        }

        T::default()
    }

    fn read_from_peer(peer: &mut TcpStream, length: usize) -> String {
        peer.set_read_timeout(Some(Duration::from_millis(READ_TIMEOUT)))
            .unwrap();

        let mut buffer = vec![0; length];
        peer.read_exact(&mut buffer).unwrap();

        String::from_utf8(buffer).unwrap()
    }

    #[test]
    fn test_connection_success() {
        let (endpoint, _peer) = create_endpoint_and_peer();

        assert!(endpoint.is_connected());
        assert!(endpoint.is_connected);
        assert!(endpoint.connection().has_stream());
    }

    #[test]
    fn test_read_string() {
        let (mut endpoint, mut peer) = create_endpoint_and_peer();

        // Nothing to read.
        assert_eq!(endpoint.read_string(), "");

        peer.write_all(b"data1\r\ndata2\r\n").unwrap();
        peer.flush().unwrap();

        // The terminator is removed, and the messages are read one by one.
        assert_eq!(wait_string(&mut endpoint), "data1");
        assert_eq!(wait_string(&mut endpoint), "data2");
        assert_eq!(endpoint.read_string(), "");
    }

    #[test]
    fn test_read_json() {
        let (mut endpoint, mut peer) = create_endpoint_and_peer();

        // Valid JSON message.
        peer.write_all(b"{\"id\":\"name\",\"value\":1}\r\n")
            .unwrap();
        peer.flush().unwrap();

        assert_eq!(wait_json(&mut endpoint), json!({"id":"name","value":1}));

        // A non-JSON message is consumed and results in null.
        peer.write_all(b"not json\r\n").unwrap();
        peer.flush().unwrap();

        assert!(wait_json(&mut endpoint).is_null());
    }

    #[test]
    fn test_read_disconnected() {
        let (mut endpoint, peer) = create_endpoint_and_peer();

        assert!(endpoint.is_connected());
        assert!(endpoint.is_connected);

        drop(peer);

        assert_eq!(wait_string(&mut endpoint), "");
        assert!(!endpoint.is_connected());
        assert!(!endpoint.is_connected);
        assert!(!endpoint.connection().has_stream());
    }

    #[test]
    fn test_write_string() {
        let (mut endpoint, mut peer) = create_endpoint_and_peer();

        endpoint.write_string(String::from("data"), true);

        assert_eq!(read_from_peer(&mut peer, 6), "data\r\n");
    }

    #[test]
    fn test_flush() {
        let (mut endpoint, mut peer) = create_endpoint_and_peer();

        endpoint.write_string(String::from("data"), false);
        peer.set_nonblocking(true).unwrap();
        sleep(Duration::from_millis(SLEEP_TIME));

        // The data is still in the buffer.
        let mut buffer = [0; 6];
        assert!(peer.read(&mut buffer).is_err());

        endpoint.flush();

        peer.set_nonblocking(false).unwrap();
        assert_eq!(read_from_peer(&mut peer, 6), "data\r\n");
    }

    #[test]
    fn test_write_json() {
        let (mut endpoint, mut peer) = create_endpoint_and_peer();

        endpoint.write_json(&json!({"id":"a"}));

        assert_eq!(read_from_peer(&mut peer, 12), "{\"id\":\"a\"}\r\n");
    }

    #[test]
    fn test_write_jsons() {
        let (mut endpoint, mut peer) = create_endpoint_and_peer();

        endpoint.write_jsons(&[json!({"id":"b"}), json!({"id":"c"})]);

        assert_eq!(
            read_from_peer(&mut peer, 24),
            "{\"id\":\"b\"}\r\n{\"id\":\"c\"}\r\n"
        );
    }

    #[test]
    fn test_write_without_connection() {
        let mut endpoint = TestEndpoint {
            connection: TcpConnection::new("endpoint", TERMINATOR),
            is_connected: false,
        };

        // Should not panic.
        endpoint.write_string(String::from("data"), true);
        endpoint.write_jsons(&[json!({"id":"a"})]);

        assert_eq!(endpoint.read_string(), "");
        assert!(endpoint.read_json().is_null());
    }

    #[test]
    fn test_close_stream() {
        let (mut endpoint, mut peer) = create_endpoint_and_peer();

        assert!(endpoint.is_connected());
        assert!(endpoint.is_connected);
        assert!(endpoint.connection().has_stream());

        endpoint.write_string(String::from("last"), false);
        endpoint.close_stream();

        assert!(!endpoint.is_connected());
        assert!(!endpoint.is_connected);
        assert!(!endpoint.connection().has_stream());

        // The buffered data is flushed before closing.
        assert_eq!(read_from_peer(&mut peer, 6), "last\r\n");

        // The peer sees the end of the stream.
        let mut buffer = [0; 1];
        assert_eq!(peer.read(&mut buffer).unwrap(), 0);
    }
}
