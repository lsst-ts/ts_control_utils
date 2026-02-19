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

use approx::assert_relative_eq;
use config::Config;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

/// Trait for parsing the configuration value.
///
/// # Parameters
/// * `Self` - Type of the configuration value.
pub trait ConfigValue: Sized {
    /// Parse the configuration value.
    ///
    /// # Parameters
    /// * `s` - String to parse.
    ///
    /// # Returns
    /// The parsed configuration value.
    fn parse_value(s: &str) -> Self;
}

/// Implement the trait ConfigValue for String.
///
/// # Parameters
/// * `String` - Type of the configuration value.
impl ConfigValue for String {
    fn parse_value(s: &str) -> Self {
        s.to_string()
    }
}

/// Implement the trait ConfigValue for f64.
///
/// # Parameters
/// * `f64` - Type of the configuration value.
impl ConfigValue for f64 {
    fn parse_value(s: &str) -> Self {
        s.parse::<f64>()
            .unwrap_or_else(|_| panic!("{s} should parse as f64"))
    }
}

/// Implement the trait ConfigValue for usize.
///
/// # Parameters
/// * `usize` - Type of the configuration value.
impl ConfigValue for usize {
    fn parse_value(s: &str) -> Self {
        s.parse::<usize>()
            .unwrap_or_else(|_| panic!("{s} should parse as usize"))
    }
}

/// Implement the trait ConfigValue for i32.
///
/// # Parameters
/// * `i32` - Type of the configuration value.
impl ConfigValue for i32 {
    fn parse_value(s: &str) -> Self {
        s.parse::<i32>()
            .unwrap_or_else(|_| panic!("{s} should parse as i32"))
    }
}

/// Implement the trait ConfigValue for u64.
///
/// # Parameters
/// * `u64` - Type of the configuration value.
///
/// # Panics
/// If the hex string does not start with 0x or 0X.
impl ConfigValue for u64 {
    fn parse_value(s: &str) -> Self {
        if !s.starts_with("0x") && !s.starts_with("0X") {
            panic!("Hex string {s} should start with 0x or 0X");
        }

        u64::from_str_radix(&s[2..], 16)
            .unwrap_or_else(|_| panic!("Hex string {s} should parse as u64"))
    }
}

/// Implement the trait ConfigValue for bool.
///
/// # Parameters
/// * `bool` - Type of the configuration value.
impl ConfigValue for bool {
    fn parse_value(s: &str) -> Self {
        s.parse::<bool>()
            .unwrap_or_else(|_| panic!("{s} should parse as bool"))
    }
}

/// Get the configuation from the file.
///
/// # Parameters
/// * `filepath` - Path to the config file.
///
/// # Returns
/// The configuration.
pub fn get_config(filepath: &Path) -> Config {
    let name = filepath
        .to_str()
        .unwrap_or_else(|| panic!("Should have the file name in the {:?}", filepath));

    Config::builder()
        .add_source(config::File::with_name(name))
        .build()
        .unwrap_or_else(|_| panic!("Should be able to read the {name}"))
}

/// Get the parameter from the file.
///
/// # Parameters
/// * `filepath` - Path to the config file.
/// * `key` - Key to find the parameter in the config file.
///
/// # Returns
/// The parameter.
pub fn get_parameter<T: ConfigValue>(filepath: &Path, key: &str) -> T {
    let config = get_config(filepath);

    config
        .get_string(key)
        .map(|v| T::parse_value(&v))
        .unwrap_or_else(|_| panic!("Should find the {key} in the {:?}", filepath))
}

/// Get the array parameter from the file.
///
/// # Parameters
/// * `filepath` - Path to the config file.
/// * `key` - Key to find the parameter in the config file.
///
/// # Returns
/// The array parameter.
pub fn get_parameter_array<T: ConfigValue>(filepath: &Path, key: &str) -> Vec<T> {
    let config = get_config(filepath);
    let config_array = config
        .get_array(key)
        .unwrap_or_else(|_| panic!("Should find the {key} in the {:?}", filepath));

    config_array
        .iter()
        .map(|x| T::parse_value(&x.clone().into_string().expect("Should be a string")))
        .collect()
}

/// Get the matrix parameter from the file.
///
/// # Parameters
/// * `filepath` - Path to the config file.
/// * `key` - Key to find the parameter in the config file.
///
/// # Returns
/// The matrix parameter.
pub fn get_parameter_matrix<T: ConfigValue>(filepath: &Path, key: &str) -> Vec<Vec<T>> {
    let config = get_config(filepath);
    let config_array = config
        .get_array(key)
        .unwrap_or_else(|_| panic!("Should find the {key} in the {:?}", filepath));

    config_array
        .iter()
        .map(|x| {
            x.clone()
                .into_array()
                .unwrap()
                .iter()
                .map(|y| T::parse_value(&y.clone().into_string().unwrap()))
                .collect()
        })
        .collect()
}

/// Assert that two vectors are equal within a relative tolerance.
///
/// # Parameters
/// * `v1` - First vector.
/// * `v2` - Second vector.
/// * `epsilon` - Relative tolerance.
///
/// # Panics
/// If the two vectors are not equal within the relative tolerance.
pub fn assert_relative_eq_vector(v1: &[f64], v2: &[f64], epsilon: f64) {
    assert_eq!(v1.len(), v2.len());
    for (a, b) in v1.iter().zip(v2.iter()) {
        assert_relative_eq!(a, b, epsilon = epsilon);
    }
}

/// TCP/IP client writes the message and sleep.
///
/// # Arguments
/// * `client` - TCP/IP client.
/// * `message` - Message to write.
/// * `sleep_time` - Sleep time in milliseconds.
///
/// # Panics
/// If the TCP stream of the client cannot write or flush.
pub fn client_write_and_sleep(client: &mut TcpStream, message: &str, sleep_time: u64) {
    client
        .write_all(message.as_bytes())
        .expect("Tcp stream should write.");
    client.flush().expect("Tcp stream should flush.");

    sleep(Duration::from_millis(sleep_time));
}

/// TCP/IP client reads the message and assert.
///
/// # Arguments
/// * `client` - TCP/IP client.
/// * `expected` - Expected message.
///
/// # Panics
/// If the TCP stream of the client cannot read.
pub fn client_read_and_assert(client: &mut TcpStream, expected: &str) {
    let mut buffer = vec![0; expected.len()];
    match client.read(&mut buffer) {
        Ok(_) => assert_eq!(std::str::from_utf8(&buffer).unwrap(), expected),
        Err(error) => panic!("{error}"),
    }
}

/// TCP/IP client reads the JSON message.
///
/// # Arguments
/// * `client` - TCP/IP client.
/// * `terminator` - Terminator of the message.
///
/// # Returns
/// JSON message.
pub fn client_read_json(client: &mut TcpStream, terminator: &[u8]) -> Value {
    let mut buffer = Vec::new();
    loop {
        let mut byte = [0; 1];
        client
            .read_exact(&mut byte)
            .expect("Tcp stream of the client should read.");

        buffer.push(byte[0]);
        if buffer.ends_with(terminator) {
            break;
        }
    }

    serde_json::from_slice(&buffer[0..(buffer.len() - terminator.len())])
        .expect("Should be able to convert to JSON.")
}

#[cfg(test)]
mod tests {
    use super::*;

    use approx::assert_relative_eq;
    use std::f64::EPSILON;
    use tempfile::Builder;

    #[test]
    fn test_get_config() {
        let mut file = Builder::new().suffix(".yaml").tempfile().unwrap();
        let _ = writeln!(file, "setting: 0.94");

        let setting = get_config(file.path()).get_float("setting").unwrap();

        assert_relative_eq!(setting, 0.94, epsilon = EPSILON);
    }

    #[test]
    fn test_get_parameter() {
        let mut file = Builder::new().suffix(".yaml").tempfile().unwrap();
        let _ = writeln!(
            file,
            "setting_str: 'abc'\nsetting_float: 0.94\nsetting_bool: true\nsetting_u64: 0xff800003fffffff8"
        );

        let filepath = file.path();

        let setting_str: String = get_parameter(filepath, "setting_str");
        assert_eq!(setting_str, "abc");

        let setting_float: f64 = get_parameter(filepath, "setting_float");
        assert_relative_eq!(setting_float, 0.94, epsilon = EPSILON);

        let setting_bool: bool = get_parameter(filepath, "setting_bool");
        assert!(setting_bool);

        let setting_u64: u64 = get_parameter(filepath, "setting_u64");
        assert_eq!(setting_u64, 0xff800003fffffff8);
    }

    #[test]
    #[should_panic(expected = "Should be able to read the wrong.yaml")]
    fn test_get_config_panic() {
        get_config(Path::new("wrong.yaml"));
    }

    #[test]
    fn test_get_parameter_array() {
        let mut file = Builder::new().suffix(".yaml").tempfile().unwrap();
        let _ = writeln!(file, "setting_array: [1.0, 2.0]");

        let setting_array: Vec<usize> = get_parameter_array(file.path(), "setting_array");

        assert_eq!(setting_array.len(), 2);
    }

    #[test]
    fn test_get_parameter_matrix() {
        let mut file = Builder::new().suffix(".yaml").tempfile().unwrap();
        let _ = writeln!(file, "setting_matrix: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]");

        let setting_matrix: Vec<Vec<f64>> = get_parameter_matrix(file.path(), "setting_matrix");

        assert_eq!(setting_matrix, vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
    }

    #[test]
    fn test_assert_relative_eq_vector() {
        assert_relative_eq_vector(&vec![1.0, 2.0, 3.0], &vec![1.0, 2.0, 3.0], EPSILON);
    }

    #[test]
    #[should_panic(expected = "`left == right` failed")]
    fn test_assert_relative_eq_vector_panic_1() {
        assert_relative_eq_vector(&vec![0.0, 0.0], &vec![0.0, 1.0, 0.0], EPSILON);
    }

    #[test]
    #[should_panic(
        expected = "assert_relative_eq!(a, b, epsilon = epsilon)\n\n    left  = 0.1\n    right = 1.1\n\n"
    )]
    fn test_assert_relative_eq_vector_panic_2() {
        assert_relative_eq_vector(&vec![0.0, 0.1, 0.0], &vec![0.0, 1.1, 0.0], EPSILON);
    }
}
