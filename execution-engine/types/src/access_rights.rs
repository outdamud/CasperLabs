use alloc::vec::Vec;

use bitflags::bitflags;

use crate::bytesrepr;

/// The number of bytes in a serialized [`AccessRights`].
pub const ACCESS_RIGHTS_SERIALIZED_LENGTH: usize = 1;

bitflags! {
    /// A struct which behaves like a set of bitflags to define access rights associated with a
    /// [`URef`](crate::URef).
    #[allow(clippy::derive_hash_xor_eq)]
    pub struct AccessRights: u8 {
        /// Permission to read the value under the associated `URef`.
        const READ  = 0b001;
        /// Permission to write a value under the associated `URef`.
        const WRITE = 0b010;
        /// Permission to add to the value under the associated `URef`.
        const ADD   = 0b100;
        /// Permission to read or add to the value under the associated `URef`.
        const READ_ADD       = Self::READ.bits | Self::ADD.bits;
        /// Permission to read or write the value under the associated `URef`.
        const READ_WRITE     = Self::READ.bits | Self::WRITE.bits;
        /// Permission to add to, or write the value under the associated `URef`.
        const ADD_WRITE      = Self::ADD.bits  | Self::WRITE.bits;
        /// Permission to read, add to, or write the value under the associated `URef`.
        const READ_ADD_WRITE = Self::READ.bits | Self::ADD.bits | Self::WRITE.bits;
    }
}

impl AccessRights {
    /// Returns `true` if the `READ` flag is set.
    pub fn is_readable(self) -> bool {
        self & AccessRights::READ == AccessRights::READ
    }

    /// Returns `true` if the `WRITE` flag is set.
    pub fn is_writeable(self) -> bool {
        self & AccessRights::WRITE == AccessRights::WRITE
    }

    /// Returns `true` if the `ADD` flag is set.
    pub fn is_addable(self) -> bool {
        self & AccessRights::ADD == AccessRights::ADD
    }
}

impl core::fmt::Display for AccessRights {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match *self {
            AccessRights::READ => write!(f, "READ"),
            AccessRights::WRITE => write!(f, "WRITE"),
            AccessRights::ADD => write!(f, "ADD"),
            AccessRights::READ_ADD => write!(f, "READ_ADD"),
            AccessRights::READ_WRITE => write!(f, "READ_WRITE"),
            AccessRights::ADD_WRITE => write!(f, "ADD_WRITE"),
            AccessRights::READ_ADD_WRITE => write!(f, "READ_ADD_WRITE"),
            _ => write!(f, "UNKNOWN"),
        }
    }
}

impl bytesrepr::ToBytes for AccessRights {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        self.bits.to_bytes()
    }
}

impl bytesrepr::FromBytes for AccessRights {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (id, rem): (u8, &[u8]) = bytesrepr::FromBytes::from_bytes(bytes)?;
        match AccessRights::from_bits(id) {
            Some(rights) => Ok((rights, rem)),
            None => Err(bytesrepr::Error::FormattingError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AccessRights;

    fn test_readable(right: AccessRights, is_true: bool) {
        assert_eq!(right.is_readable(), is_true)
    }

    #[test]
    fn test_is_readable() {
        test_readable(AccessRights::READ, true);
        test_readable(AccessRights::READ_ADD, true);
        test_readable(AccessRights::READ_WRITE, true);
        test_readable(AccessRights::READ_ADD_WRITE, true);
        test_readable(AccessRights::ADD, false);
        test_readable(AccessRights::ADD_WRITE, false);
        test_readable(AccessRights::WRITE, false);
    }

    fn test_writable(right: AccessRights, is_true: bool) {
        assert_eq!(right.is_writeable(), is_true)
    }

    #[test]
    fn test_is_writable() {
        test_writable(AccessRights::WRITE, true);
        test_writable(AccessRights::READ_WRITE, true);
        test_writable(AccessRights::ADD_WRITE, true);
        test_writable(AccessRights::READ, false);
        test_writable(AccessRights::ADD, false);
        test_writable(AccessRights::READ_ADD, false);
        test_writable(AccessRights::READ_ADD_WRITE, true);
    }

    fn test_addable(right: AccessRights, is_true: bool) {
        assert_eq!(right.is_addable(), is_true)
    }

    #[test]
    fn test_is_addable() {
        test_addable(AccessRights::ADD, true);
        test_addable(AccessRights::READ_ADD, true);
        test_addable(AccessRights::READ_WRITE, false);
        test_addable(AccessRights::ADD_WRITE, true);
        test_addable(AccessRights::READ, false);
        test_addable(AccessRights::WRITE, false);
        test_addable(AccessRights::READ_ADD_WRITE, true);
    }
}
