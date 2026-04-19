//! The `BlockKind` enum — a tagged union of every block type in
//! this implementation. Each variant implements `Block`.

use crate::keyboard::blocks::{ColStagBlock, FreeFormBlock, RowStagBlock};
use crate::keyboard::common::PhysicalKey;
use crate::keyboard::{Block, StaggerType};

#[derive(Debug, Clone)]
pub enum BlockKind {
    RowStag(RowStagBlock),
    ColStag(ColStagBlock),
    FreeForm(FreeFormBlock),
}

impl Block for BlockKind {
    fn stagger_type(&self) -> StaggerType {
        match self {
            BlockKind::RowStag(b) => b.stagger_type(),
            BlockKind::ColStag(b) => b.stagger_type(),
            BlockKind::FreeForm(b) => b.stagger_type(),
        }
    }

    fn cluster(&self) -> &str {
        match self {
            BlockKind::RowStag(b) => b.cluster(),
            BlockKind::ColStag(b) => b.cluster(),
            BlockKind::FreeForm(b) => b.cluster(),
        }
    }

    fn keys(&self) -> Box<dyn Iterator<Item = &PhysicalKey> + '_> {
        match self {
            BlockKind::RowStag(b) => b.keys(),
            BlockKind::ColStag(b) => b.keys(),
            BlockKind::FreeForm(b) => b.keys(),
        }
    }
}
