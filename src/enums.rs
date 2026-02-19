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

use num_traits::PrimInt;

/// A trait to provide value and bit value methods for the bit enum.
pub trait BitEnum<T: PrimInt> {
    /// Get the value.
    ///
    /// # Returns
    /// Value.
    fn value(&self) -> T;

    /// Get the bit value.
    ///
    /// # Returns
    /// Bit value. If the value is not defined, it returns 0.
    fn bit_value(&self) -> T {
        match self.value().to_usize() {
            Some(value) => T::one() << value,
            None => T::zero(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    pub enum TestCode {
        A = 0,
        B = 1,
        C = 2,
        D = 3,
    }

    impl BitEnum<u64> for TestCode {
        fn value(&self) -> u64 {
            *self as u64
        }
    }

    #[test]
    fn test_error_code_bit_value() {
        assert_eq!(TestCode::A.bit_value(), 1);
        assert_eq!(TestCode::B.bit_value(), 2);
        assert_eq!(TestCode::C.bit_value(), 4);
        assert_eq!(TestCode::D.bit_value(), 8);
    }
}
