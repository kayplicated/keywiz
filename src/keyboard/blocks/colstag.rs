//! Column-stagger block — columns are rigid vertical units.
//!
//! On split ergonomic boards (kyria, elora, halcyon), each column
//! slides up or down relative to the adjacent column — the middle
//! finger column thrust forward, the pinky pulled back. Terminal
//! *flattens* these offsets (chars are too chunky to render
//! fractional y cleanly), using integer `r`/`c` for placement.
//! Desktop/webui renders the real y.

use crate::keyboard::common::PhysicalKey;
use crate::keyboard::{Block, StaggerType};

#[derive(Debug, Clone)]
pub struct ColStagBlock {
    pub cluster: String,
    pub keys: Vec<PhysicalKey>,
}

impl Block for ColStagBlock {
    fn stagger_type(&self) -> StaggerType {
        StaggerType::ColStag
    }

    fn cluster(&self) -> &str {
        &self.cluster
    }

    fn keys(&self) -> Box<dyn Iterator<Item = &PhysicalKey> + '_> {
        Box::new(self.keys.iter())
    }
}
