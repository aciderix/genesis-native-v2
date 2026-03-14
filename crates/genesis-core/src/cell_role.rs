/// Cell differentiation roles within groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CellRole {
    /// Not yet differentiated.
    Undifferentiated,
    /// Interior cell (surrounded by bonds).
    Interior,
    /// Border cell (fewer bonds, exposed to environment).
    Border,
}

impl Default for CellRole {
    fn default() -> Self {
        CellRole::Undifferentiated
    }
}

impl CellRole {
    pub fn as_index(&self) -> usize {
        match self {
            CellRole::Undifferentiated => 0,
            CellRole::Interior => 1,
            CellRole::Border => 2,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            1 => CellRole::Interior,
            2 => CellRole::Border,
            _ => CellRole::Undifferentiated,
        }
    }
}
