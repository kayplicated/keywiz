//! Free-form block — keys placed by explicit xy + rotation.
//!
//! Fan-shaped thumb clusters, macropads with unusual arrangements,
//! anything that doesn't decompose into rows-or-columns. Desktop and
//! webui renderers use xy + rotation to draw the true shape;
//! terminal treats free-form like col-stagger (use r/c, flatten
//! geometric offsets) so the thumb cluster renders as a clean flat
//! block beneath main.

use crate::keyboard::common::PhysicalKey;
use crate::keyboard::{Block, StaggerType};

#[derive(Debug, Clone)]
pub struct FreeFormBlock {
    /// Staged for future per-cluster theming / addressing.
    #[allow(dead_code)]
    pub cluster: String,
    pub keys: Vec<PhysicalKey>,
}

impl Block for FreeFormBlock {
    fn stagger_type(&self) -> StaggerType {
        StaggerType::FreeForm
    }

    fn cluster(&self) -> &str {
        &self.cluster
    }

    fn keys(&self) -> Box<dyn Iterator<Item = &PhysicalKey> + '_> {
        Box::new(self.keys.iter())
    }
}
