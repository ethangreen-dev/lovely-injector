use itertools::Itertools;

// Extension trait
pub trait IntoCursor {
    type Cursor;
    fn into_cursor(self) -> Self::Cursor;
}

pub struct ChunkVecCursor<'a> {
    chunks: Vec<&'a str>,
    idx: usize,
    len: usize,
    offset: usize,
}

impl<'a> regex_cursor::Cursor for ChunkVecCursor<'a> {
    fn chunk(&self) -> &[u8] {
        self.chunks[self.idx].as_bytes()
    }

    fn advance(&mut self) -> bool {
        if self.idx + 1 >= self.chunks.len() {
            return false
        }
        self.offset += self.chunks[self.idx].len();
        self.idx += 1;
        true
    }

    fn backtrack(&mut self) -> bool {
        if self.idx == 0 {
            return false
        }
        self.idx -= 1;
        self.offset -= self.chunks[self.idx].len();
        true
    }

    fn total_bytes(&self) -> Option<usize> {
        Some(self.len)
    }

    fn offset(&self) -> usize {
        self.offset
    }
}

impl<'a> IntoCursor for &'a crop::Rope {
    type Cursor = ChunkVecCursor<'a>;
    
    fn into_cursor(self) -> Self::Cursor {
        ChunkVecCursor {
            chunks: self.chunks().collect_vec(),
            idx: 0,
            len: self.byte_len(),
            offset: 0,
        }
    }
}

impl<'a> IntoCursor for crop::RopeSlice<'a> {
    type Cursor = ChunkVecCursor<'a>;
    
    fn into_cursor(self) -> Self::Cursor {
        ChunkVecCursor {
            chunks: self.chunks().collect_vec(),
            idx: 0,
            len: self.byte_len(),
            offset: 0,
        }
    }
}