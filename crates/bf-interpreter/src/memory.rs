pub const TAPE_SIZE: usize = 30_000;

pub struct Memory {
    cells: Vec<u8>,
    pointer: usize,
}

impl Memory {
    pub fn new() -> Self {
        Memory {
            cells: vec![0u8; TAPE_SIZE],
            pointer: 0,
        }
    }

    pub fn get(&self) -> u8 {
        self.cells[self.pointer]
    }

    pub fn set(&mut self, val: u8) {
        self.cells[self.pointer] = val;
    }

    pub fn inc(&mut self) {
        self.cells[self.pointer] = self.cells[self.pointer].wrapping_add(1);
    }

    pub fn dec(&mut self) {
        self.cells[self.pointer] = self.cells[self.pointer].wrapping_sub(1);
    }

    /// Move pointer left. Caller must verify pointer > 0.
    pub fn left(&mut self) {
        self.pointer -= 1;
    }

    /// Move pointer right. Caller must verify pointer + 1 < TAPE_SIZE.
    pub fn right(&mut self) {
        self.pointer += 1;
    }

    pub fn pointer(&self) -> usize {
        self.pointer
    }

    pub fn cells(&self) -> &[u8] {
        &self.cells
    }

    pub fn cells_as_mut_ptr(&mut self) -> *mut u8 {
        self.cells.as_mut_ptr()
    }

    pub fn set_pointer(&mut self, p: usize) {
        self.pointer = p;
    }
}
