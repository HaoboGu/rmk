pub enum HandFlags {
    Left,
    Right,
    None,
}
pub struct ChordalHoldMapConfig<'a, const ROW: usize, const COL: usize> {
    // a matrix to store chordal hold flags for every key
    pub(crate) matrix: &'a mut [[[HandFlags; COL]; ROW]],
}