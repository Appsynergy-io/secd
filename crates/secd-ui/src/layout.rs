//! Desktop ≥900 is list|inspector. Below 900 is list → sheet.

use crate::tokens::BREAKPOINT_PX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutMode {
    ListOnly,
    ListInspector,
}

impl LayoutMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ListOnly => "list-only",
            Self::ListInspector => "list-inspector",
        }
    }

    pub fn shows_inspector(self) -> bool {
        matches!(self, Self::ListInspector)
    }

    pub fn uses_sheet(self) -> bool {
        matches!(self, Self::ListOnly)
    }
}

pub fn layout_mode(width_px: u32) -> LayoutMode {
    if width_px >= BREAKPOINT_PX {
        LayoutMode::ListInspector
    } else {
        LayoutMode::ListOnly
    }
}
