#![allow(dead_code)]

use std::{error::Error, os::unix::fs::FileExt};

pub const PAGE_SIZE: usize = 4096;
const SLOT_SIZE: usize = 4; // cell_offset: u16 + cell_size: u16
const HEADER_SIZE: usize = 4 + 1 + 2 + 2 + 2 + 1; // for the calculation look at the PageHeader struct for data members

/// This is a B*-tree implementation
struct Btree {
    root: Option<Node>,
}

/// the actual page mapping 1:1 to the Node
struct Node {
    page: Page,
    degree: usize,
    keys: i64,          // TODO: allow generics, add flexible comparator
    children: Vec<u32>, // page ids of children
}

/// Layout will be like: \[header\]—\[p1\]—\[p2\]—\[free_space\]—\[cell2\]—\[cell2\]
struct Page {
    data: [u8; PAGE_SIZE], // LE
}

#[repr(u8)]
#[derive(Clone, Copy)]
enum PageKind {
    Root = 0,
    Internal = 1,
    Leaf = 2,
}

impl PageKind {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(PageKind::Root),
            1 => Some(PageKind::Internal),
            2 => Some(PageKind::Leaf),
            _ => None,
        }
    }
}

struct PageHeader {
    id: u32,
    page_ty: PageKind,
    free_start: u16,
    free_end: u16,
    total_size: u16,
    flags: u8,
}

impl PageHeader {
    pub fn serialize(&self, buf: &mut [u8]) {
        buf[0..4].copy_from_slice(&self.id.to_le_bytes());
        // buf[4] = (&self.page_ty as *const PageKind) as u8;
        buf[4] = self.page_ty as u8;
        buf[5..7].copy_from_slice(&self.free_start.to_le_bytes());
        buf[7..9].copy_from_slice(&self.free_end.to_le_bytes());
        buf[9..11].copy_from_slice(&self.total_size.to_le_bytes());
        buf[11] = self.flags;
    }

    pub fn deserialize(buf: &[u8]) -> Option<Self> {
        Some(PageHeader {
            id: u32::from_le_bytes(buf[0..4].try_into().ok()?),
            page_ty: PageKind::from_u8(buf[4])?,
            free_start: u16::from_le_bytes(buf[5..7].try_into().ok()?),
            free_end: u16::from_le_bytes(buf[7..9].try_into().ok()?),
            total_size: u16::from_le_bytes(buf[9..11].try_into().ok()?),
            flags: u8::from_le(buf[11]),
        })
    }
}

/// starting offset → \[Cell\] ← ending offset (ending is the cell_offset)
struct CellPointer {
    cell_offset: u16,
    cell_size: u16,
}

impl Page {
    pub fn add_cell(&mut self, c_data: &[u8]) -> Result<(), Box<dyn Error>> {
        let cell_size = c_data.len();

        let mut hdr = PageHeader::deserialize(&self.data[..HEADER_SIZE]).unwrap();

        let p_offset = hdr.free_start;
        let c_offset = hdr.free_end;

        let end = c_offset as usize;

        // NOTE: overflow page not implemented yet, for now just panic
        let start = end
            .checked_sub(cell_size)
            .filter(|&s| s >= p_offset as usize)
            .ok_or("OH NO! Page overflow! can't insert >w<")?;

        self.data[start..end].clone_from_slice(c_data);

        // NOTE: for cell pointer layout im thinking prolly just a 4 byte long sequence(cell_offset: u16, cell_size: u16) after header
        let cell = CellPointer {
            cell_offset: end as u16,
            cell_size: cell_size as u16,
        };

        self.data[p_offset as usize..p_offset as usize + 2]
            .copy_from_slice(&cell.cell_offset.to_le_bytes());
        self.data[p_offset as usize + 2..p_offset as usize + 4]
            .copy_from_slice(&cell.cell_size.to_le_bytes());

        hdr.free_start += 4;
        hdr.free_end = start as u16;
        hdr.serialize(&mut self.data[..HEADER_SIZE]);

        Ok(())
    }

    fn num_slots(&mut self) -> u16 {
        let hdr = self.header();
        (hdr.free_start - HEADER_SIZE as u16) / SLOT_SIZE as u16
    }

    fn slot(&self, i: u16) -> CellPointer {
        let off = HEADER_SIZE + i as usize * SLOT_SIZE;

        CellPointer {
            cell_offset: u16::from_le_bytes(self.data[off..off + 2].try_into().unwrap()),
            cell_size: u16::from_le_bytes(self.data[off + 2..off + 4].try_into().unwrap()),
        }
    }

    fn cell(&self, i: u16) -> &[u8] {
        let s = self.slot(i);
        let start = s.cell_offset as usize;
        &self.data[start..start + s.cell_size as usize]
    }

    fn header(&self) -> PageHeader {
        PageHeader::deserialize(&self.data[..HEADER_SIZE]).unwrap()
    }
}

struct Pager {
    file: std::fs::File,
    frames: std::collections::HashMap<u32, Page>,
    next_id: u32,
}

impl Pager {
    fn fetch(&mut self, id: u32) -> &mut Page {
        self.frames.entry(id).or_insert_with(|| {
            let mut buf = [0u8; PAGE_SIZE];
            self.file
                .read_exact_at(&mut buf, Self::page_offset(id))
                .unwrap();
            Page { data: buf }
        })
    }

    fn allocate(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn page_offset(id: u32) -> u64 {
        id as u64 * PAGE_SIZE as u64
    }
}
