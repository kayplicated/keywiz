//! Row-stagger block — rows are rigid horizontal units.
//!
//! On ANSI keyboards, each row slides left or right relative to the
//! row above (e.g. the top alpha row on an ANSI board is shifted
//! ~0.5 key-widths right of the home row). Terminal renders this
//! faithfully using fractional x; col-stagger boards get a flat row
//! each because their stagger is on the other axis.

use crate::keyboard::common::PhysicalKey;
use crate::keyboard::{Block, StaggerType};

#[derive(Debug, Clone)]
pub struct RowStagBlock {
    /// Staged for future per-cluster theming / addressing.
    #[allow(dead_code)]
    pub cluster: String,
    pub keys: Vec<PhysicalKey>,
}

impl Block for RowStagBlock {
    fn stagger_type(&self) -> StaggerType {
        StaggerType::RowStag
    }

    fn cluster(&self) -> &str {
        &self.cluster
    }

    fn keys(&self) -> Box<dyn Iterator<Item = &PhysicalKey> + '_> {
        Box::new(self.keys.iter())
    }
}
