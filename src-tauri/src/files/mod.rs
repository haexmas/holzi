//! Files the person hands to holzi (spec 043): chosen in the system's dialog, read and written
//! the same way on every platform.

pub mod commands;
pub mod picked;

pub use picked::PickedFile;

#[cfg(test)]
pub mod test_support;
