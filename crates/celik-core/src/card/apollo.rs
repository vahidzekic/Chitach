//! Apollo ID cards (the first generation of smart ID cards).
//!
//! Port of bas-celik `card/apollo.go` (MIT, (c) Nikola Ubavić).

use super::id::IdFiles;
use super::{read, read_chunks, select, CardDocument, CardKind, CardTransport};
use crate::document::Document;
use crate::{CardError, Result};

/// Apollo cards are not manufactured anymore.
pub struct Apollo<'a> {
    atr: Vec<u8>,
    card: &'a dyn CardTransport,
    files: IdFiles,
}

impl<'a> Apollo<'a> {
    pub fn new(card: &'a dyn CardTransport, atr: Vec<u8>) -> Self {
        Self {
            atr,
            card,
            files: IdFiles::default(),
        }
    }

    /// Reads a file: 6 byte header with the little-endian data length at offset 4.
    pub fn read_file(&self, name: &[u8]) -> Result<Vec<u8>> {
        select(self.card, 0x08, 0x00, name, 4)?;

        let data = read(self.card, 0, 6)?;
        if data.len() < 6 {
            return Err(CardError::Parse("file too short".into()));
        }
        let length = u16::from_le_bytes([data[4], data[5]]) as usize;
        read_chunks(self.card, 6, length, 0xFF)
    }
}

impl CardDocument for Apollo<'_> {
    fn kind(&self) -> CardKind {
        CardKind::ApolloId
    }
    fn atr(&self) -> &[u8] {
        &self.atr
    }
    fn init_card(&mut self) -> Result<()> {
        Ok(())
    }
    fn read_card(&mut self) -> Result<()> {
        self.files = IdFiles::read(|n| self.read_file(n))?;
        Ok(())
    }
    fn get_document(&self) -> Result<Document> {
        Ok(Document::Id(Box::new(self.files.parse()?)))
    }
    fn test(&mut self) -> bool {
        true
    }
}
