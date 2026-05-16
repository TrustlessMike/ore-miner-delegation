use solana_program::{pubkey, pubkey::Pubkey};

pub const MANAGED_PROOF: &[u8] = b"managed-proof-account";
pub const DELEGATED_STAKE: &[u8] = b"delegated-stake";
pub const DELEGATED_BOOST: &[u8] = b"delegated-boost";
pub const DELEGATED_BOOST_V2: &[u8] = b"v2-delegated-boost";

pub const LEGACY_BOOST_PROGRAM_ID: Pubkey =
    pubkey!("boostmPwypNUQu8qZ8RoWt5DXyYSVYxnBXqbbrGjecc");
