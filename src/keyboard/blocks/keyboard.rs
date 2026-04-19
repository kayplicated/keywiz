//! Concrete blocks-based keyboard. Implements `trait Keyboard`.

use crate::keyboard::blocks::BlockKind;
use crate::keyboard::common::{Bounds, PhysicalKey, Point};
use crate::keyboard::{Block, Keyboard};

#[derive(Debug, Clone)]
pub struct BlocksKeyboard {
    pub name: String,
    pub short: String,
    pub description: String,
    pub blocks: Vec<BlockKind>,
}

impl Keyboard for BlocksKeyboard {
    fn name(&self) -> &str {
        &self.name
    }

    fn short(&self) -> &str {
        &self.short
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn keys(&self) -> Box<dyn Iterator<Item = &PhysicalKey> + '_> {
        Box::new(self.blocks.iter().flat_map(|b| b.keys()))
    }

    fn key(&self, id: &str) -> Option<&PhysicalKey> {
        self.keys().find(|k| k.id == id)
    }

    fn blocks(&self) -> Box<dyn Iterator<Item = &dyn Block> + '_> {
        Box::new(self.blocks.iter().map(|b| b as &dyn Block))
    }

    fn bounds(&self) -> Bounds {
        Bounds::enclosing(self.keys().map(|k| Point::new(k.x, k.y)))
    }
}
