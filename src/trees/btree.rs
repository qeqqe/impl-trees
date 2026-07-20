#![allow(dead_code)]

use core::fmt;
use std::{
    cell::{RefCell, RefMut},
    collections::{HashMap, VecDeque},
    error::Error,
    os::unix::fs::FileExt,
    path::PathBuf,
};

pub const PAGE_SIZE: usize = 4096;
const SLOT_SIZE: usize = 4; // cell_offset: u16 + cell_size: u16
const HEADER_SIZE: usize = 4 + 1 + 2 + 2 + 2 + 1 + 4; // for the calculation look at the PageHeader struct for data members
const DEGREE: usize = 3; // 4 keys per node at max, 2 keys min

/// This is a B-tree implementation
pub struct Btree {
    pub root_id: u32,
    pub pager: Pager,
}

impl fmt::Display for Btree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.pretty_print().unwrap();
        write!(f, "")
    }
}

impl Btree {
    pub fn new(val: i64, path_buf: PathBuf) -> Result<Self, Box<dyn Error>> {
        let file = match std::fs::File::open(&path_buf) {
            Ok(f) => f,
            _ => std::fs::File::create(&path_buf).unwrap(),
        };

        let mut pager = Pager {
            file,
            frames: HashMap::new().into(),
            path_buf,
            next_id: 1,
        };
        let root_id = pager.allocate();
        pager
            .fetch(root_id)
            .reset_and_fill(root_id, PageKind::Leaf, 0, &[])?;
        pager.fetch(root_id).add_cell(&val.to_le_bytes())?;

        Ok(Btree { root_id, pager })
    }

    pub fn pretty_print(&self) -> Result<(), Box<dyn Error>> {
        let mut queue = VecDeque::new();
        queue.push_back(self.root_id);
        let mut level: usize = 0;

        while !queue.is_empty() {
            let level_size = queue.len();

            print!("level {level}: ");

            for _ in 0..level_size {
                let page_id = queue.pop_front().ok_or("Cell not found".to_string())?;
                let page = self.pager.fetch(page_id);
                let cells = page.get_cells()?;
                let p_hdr = page.header()?;

                print!("[");
                for cell in &cells {
                    print!(" {} ", cell.key);
                }
                print!("]");

                if p_hdr.page_ty != PageKind::Leaf {
                    let childs: Vec<u32> = cells.iter().map(|c| c.c_ptr.unwrap()).collect();
                    for c_ptr in childs {
                        queue.push_back(c_ptr);
                    }
                    queue.push_back(p_hdr.rightmost_ptr);
                }
            }
            println!();

            level += 1;
        }

        Ok(())
    }

    pub fn search(&self, val: i64) -> Result<bool, Box<dyn Error>> {
        let mut page_id = self.root_id;
        loop {
            let (cells, p_hdr) = {
                let page = self.pager.fetch(page_id);
                let p_hdr = page.header()?;
                (page.get_cells()?, p_hdr)
            };
            match cells.binary_search_by(|cell| cell.key.cmp(&val)) {
                Ok(_) => return Ok(true),
                Err(i) => {
                    if p_hdr.page_ty == PageKind::Leaf {
                        return Ok(false);
                    }

                    let child_page_id = if i < cells.len() {
                        cells.get(i).unwrap().c_ptr.unwrap()
                    } else {
                        p_hdr.rightmost_ptr
                    };
                    page_id = child_page_id;
                }
            };
        }
    }

    fn get_insertion_breadcrumbs(&mut self, val: i64) -> Result<Vec<u32>, Box<dyn Error>> {
        let mut breadcrumb: Vec<u32> = Vec::new(); // stack for tracing the descend path

        let mut page_id = self.root_id;
        // descend till the leaf node, and insert in the right position.
        // we handle overflow
        loop {
            let (cells, p_hdr) = {
                let page = self.pager.fetch(page_id);
                let p_hdr = page.header()?;
                (page.get_cells()?, p_hdr)
            };
            breadcrumb.push(p_hdr.id);
            match cells.binary_search_by(|cell| cell.key.cmp(&val)) {
                Ok(_) => return Err("Already exists".into()), //  TODO: handle dupes
                Err(i) => {
                    if p_hdr.page_ty == PageKind::Leaf {
                        return Ok(breadcrumb);
                    } else {
                        let child_page_id = if i < cells.len() {
                            cells.get(i).unwrap().c_ptr.unwrap()
                        } else {
                            p_hdr.rightmost_ptr
                        };
                        page_id = child_page_id;
                    }
                }
            };
        }
    }

    pub fn insert(&mut self, val: i64) -> Result<(), Box<dyn Error>> {
        let mut breadcrumb = self.get_insertion_breadcrumbs(val)?;
        let Some(page_id) = breadcrumb.last().copied() else {
            return Err("No page found".into());
        };

        let n_slots = {
            let mut page = self.pager.fetch(page_id);
            let c_data = val.to_le_bytes();
            page.add_cell(&c_data)?
        };

        if n_slots >= 2 * DEGREE - 1 {
            let try_root_id = self.handle_overflow(&mut breadcrumb)?;
            match try_root_id {
                Some(new_root_id) => self.root_id = new_root_id,
                _ => return Ok(()),
            }
        }

        Ok(())
    }

    pub fn handle_overflow(
        &mut self,
        breadcrumb: &mut Vec<u32>,
    ) -> Result<Option<u32>, Box<dyn Error>> {
        let Some(overflow_page_id) = breadcrumb.pop() else {
            return Err("Current Page id itself not found".into()); // this error shouldnt even trigger
        };

        let (current_hdr, current_cells) = {
            let page = self.pager.fetch(overflow_page_id);
            let hdr = page.header()?;
            let cells = page.get_cells()?;
            (hdr, cells)
        };

        let n = current_cells.len();
        let split = n / 2;

        // we can say that if the first element does not have a child it's either a leaf or a
        // variant of root which is not big enough to have childs
        let no_child = match current_cells.first() {
            Some(c) => !c.c_ptr.is_some(),
            _ => unreachable!(),
        };

        // so first of all we will split the CURRENT page into three parts
        // [LEFT] || [MEDIAN] || [RIGHT]
        // after this we can create the LEFT and RIGHT into its individual pages/nodes,
        // (im think thinking keeping the left as is in the same page and create a new page for the
        // RIGHT).
        // so Page Left and Page Right, store the page_id of both trees.
        // Now with the median pull it to the PARENT, and attach LEFT Page to the child pointer of
        // the MEDIAN cell, and Cell next to the median will store the Right page.
        // IF there's no Cell right to the median then store it as a RIGHT POINTER to the Page in
        // the header.
        // NOTE: Coalescing the empty page will only break page offsets and adds unncessary complexity
        // So we leave it as is.

        let (left_cells, right_cells, promoted_key, left_rightmost) = if no_child {
            (
                &current_cells[0..split],
                &current_cells[split + 1..],
                current_cells[split].key,
                0u32, // unused for leaves
            )
        } else {
            let median = &current_cells[split];
            (
                &current_cells[0..split],
                &current_cells[split + 1..],
                median.key,
                median.c_ptr.ok_or("internal cell missing c_ptr")?,
            )
        };

        // rebuild LEFT in place
        self.pager.fetch(overflow_page_id).reset_and_fill(
            overflow_page_id,
            current_hdr.page_ty,
            left_rightmost,
            left_cells,
        )?;

        // move RIGHT to new page
        let right_page_id = self.pager.allocate();
        let right_rightmost = if no_child {
            0
        } else {
            current_hdr.rightmost_ptr
        };

        self.pager.fetch(right_page_id).reset_and_fill(
            right_page_id,
            current_hdr.page_ty,
            right_rightmost,
            right_cells,
        )?;

        let Some(parent_id) = breadcrumb.pop() else {
            // because there's no parent for the root, we extend the tree level by 1
            let new_root_id = self.pager.allocate();

            self.pager.fetch(new_root_id).reset_and_fill(
                new_root_id,
                PageKind::Root,
                right_page_id,
                &[],
            )?;

            let mut buf = Vec::with_capacity(12);
            buf.extend_from_slice(&promoted_key.to_le_bytes());
            buf.extend_from_slice(&overflow_page_id.to_le_bytes());

            self.pager.fetch(new_root_id).add_cell(&buf)?;

            self.pager
                .fetch(overflow_page_id)
                .set_page_ty(if no_child {
                    PageKind::Leaf
                } else {
                    PageKind::Internal
                })?;

            return Ok(Some(new_root_id));
        };

        // repoint whichever pointer in parent used to reference overflow_page_id (now LEFT) to RIGHT
        let parent_hdr = self.pager.fetch(parent_id).header()?;
        let parent_n_slots = if parent_hdr.rightmost_ptr == overflow_page_id {
            self.pager
                .fetch(parent_id)
                .set_rightmost_ptr(right_page_id)?;

            let mut buf = Vec::with_capacity(12);
            buf.extend_from_slice(&promoted_key.to_le_bytes());
            buf.extend_from_slice(&overflow_page_id.to_le_bytes());

            self.pager.fetch(parent_id).add_cell(&buf)?
        } else {
            let parent_cells = self.pager.fetch(parent_id).get_cells()?;
            let idx = parent_cells
                .iter()
                .position(|c| c.c_ptr == Some(overflow_page_id))
                .ok_or("Parent doesn't reference it's overflowing child, tree done fcked up :/")?;
            let mut buf = Vec::with_capacity(12);

            // (pormoted_key, LEFT's id)
            buf.extend_from_slice(&promoted_key.to_le_bytes());
            buf.extend_from_slice(&overflow_page_id.to_le_bytes());

            self.pager
                .fetch(parent_id)
                .set_child_ptr_at(idx, right_page_id)?;
            self.pager.fetch(parent_id).add_cell(&buf)?
        };

        if parent_n_slots >= 2 * DEGREE - 1 {
            breadcrumb.push(parent_id);
            return self.handle_overflow(breadcrumb);
        }

        Ok(None)
    }

    fn get_deletion_breadcrumbs(&mut self, val: i64) -> Result<(Vec<u32>, usize), Box<dyn Error>> {
        let mut breadcrumb: Vec<u32> = Vec::new(); // stack for tracing the descend path

        let mut page_id = self.root_id;
        // descend till the leaf node, and insert in the right position.
        // we handle overflow
        loop {
            let (cells, p_hdr) = {
                let page = self.pager.fetch(page_id);
                let p_hdr = page.header()?;
                (page.get_cells()?, p_hdr)
            };
            breadcrumb.push(p_hdr.id);
            match cells.binary_search_by(|cell| cell.key.cmp(&val)) {
                Ok(i) => return Ok((breadcrumb, i)),
                Err(i) => {
                    if p_hdr.page_ty == PageKind::Leaf {
                        return Err("Item doesn't exist".into());
                    } else {
                        let child_page_id = if i < cells.len() {
                            cells.get(i).unwrap().c_ptr.unwrap()
                        } else {
                            p_hdr.rightmost_ptr
                        };
                        page_id = child_page_id;
                    }
                }
            };
        }
    }

    pub fn remove(&mut self, val: i64) -> Result<bool, Box<dyn Error>> {
        let (mut breadcrumbs, idx) = match self.get_deletion_breadcrumbs(val) {
            Ok(res) => res,
            Err(_) => return Ok(false),
        };

        let mut leaf_idx = idx;
        let mut target_page_id = *breadcrumbs.last().unwrap();

        let target_hdr = self.pager.fetch(target_page_id).header()?;
        if target_hdr.page_ty != PageKind::Leaf {
            // predecessor leaf swap swap
            let target_cells = self.pager.fetch(target_page_id).get_cells()?;
            let left_child_id = target_cells[idx]
                .c_ptr
                .ok_or("internal cell missing child ptr")?;
            breadcrumbs.push(left_child_id);

            let mut curr_id = left_child_id;
            loop {
                let curr_hdr = self.pager.fetch(curr_id).header()?;
                if curr_hdr.page_ty == PageKind::Leaf {
                    break;
                }
                let next_id = curr_hdr.rightmost_ptr;
                breadcrumbs.push(next_id);
                curr_id = next_id;
            }

            let leaf_id = curr_id;
            let leaf_cells = self.pager.fetch(leaf_id).get_cells()?;
            leaf_idx = leaf_cells.len() - 1;
            let pred_key = leaf_cells[leaf_idx].key;

            // key swap in parent
            {
                let mut internal_page = self.pager.fetch(target_page_id);
                let slot = internal_page.slot(idx as u16);
                let start = slot.cell_offset as usize;
                internal_page.data[start..start + 8].copy_from_slice(&pred_key.to_le_bytes());
            }

            target_page_id = leaf_id;
        }

        self.pager.fetch(target_page_id).remove_cell(leaf_idx)?;

        self.handle_underflow(&mut breadcrumbs)?;

        Ok(true)
    }

    pub fn handle_underflow(&mut self, breadcrumbs: &mut Vec<u32>) -> Result<(), Box<dyn Error>> {
        let Some(page_id) = breadcrumbs.pop() else {
            return Ok(());
        };

        let cell_count = self.pager.fetch(page_id).get_cells()?.len();
        let hdr = self.pager.fetch(page_id).header()?;

        if page_id == self.root_id {
            if cell_count == 0 && hdr.page_ty != PageKind::Leaf {
                // root is empty promote rightmost ptr to root
                let new_root_id = hdr.rightmost_ptr;
                self.root_id = new_root_id;
                self.pager.remove_entry(page_id);
            }
            return Ok(());
        }

        if cell_count >= DEGREE - 1 {
            return Ok(());
        }

        let Some(&parent_id) = breadcrumbs.last() else {
            return Err("Parent not found in breadcrumbs for underflow".into());
        };

        let parent_hdr = self.pager.fetch(parent_id).header()?;
        let parent_cells = self.pager.fetch(parent_id).get_cells()?;

        let mut children = Vec::new();
        for cell in &parent_cells {
            children.push(cell.c_ptr.unwrap());
        }
        children.push(parent_hdr.rightmost_ptr);

        let child_idx = children
            .iter()
            .position(|&id| id == page_id)
            .ok_or("Child not found in parent")?;

        // sibling borrowing check
        if child_idx > 0 {
            let left_sibling_id = children[child_idx - 1];
            let left_cells = self.pager.fetch(left_sibling_id).get_cells()?;
            if left_cells.len() > DEGREE - 1 {
                self.borrow_from_left(page_id, left_sibling_id, parent_id, child_idx - 1)?;
                return Ok(());
            }
        }

        if child_idx + 1 < children.len() {
            let right_sibling_id = children[child_idx + 1];
            let right_cells = self.pager.fetch(right_sibling_id).get_cells()?;
            if right_cells.len() > DEGREE - 1 {
                self.borrow_from_right(page_id, right_sibling_id, parent_id, child_idx)?;
                return Ok(());
            }
        }

        // merge
        if child_idx > 0 {
            let left_sibling_id = children[child_idx - 1];
            self.merge_nodes(left_sibling_id, page_id, parent_id, child_idx - 1)?;
        } else {
            let right_sibling_id = children[child_idx + 1];
            self.merge_nodes(page_id, right_sibling_id, parent_id, child_idx)?;
        }

        self.handle_underflow(breadcrumbs)?;
        Ok(())
    }

    fn borrow_from_left(
        &mut self,
        page_id: u32,
        left_id: u32,
        parent_id: u32,
        parent_key_idx: usize,
    ) -> Result<(), Box<dyn Error>> {
        let parent_key = self.pager.fetch(parent_id).get_cells()?[parent_key_idx].key;
        let left_cells = self.pager.fetch(left_id).get_cells()?;
        let left_last_cell = &left_cells[left_cells.len() - 1];
        let sibling_last_key = left_last_cell.key;
        let sibling_last_ptr = left_last_cell.c_ptr;

        let left_hdr = self.pager.fetch(left_id).header()?;
        let sibling_rightmost = left_hdr.rightmost_ptr;

        // sibling last key goes up
        {
            let mut parent_page = self.pager.fetch(parent_id);
            let slot = parent_page.slot(parent_key_idx as u16);
            let start = slot.cell_offset as usize;
            parent_page.data[start..start + 8].copy_from_slice(&sibling_last_key.to_le_bytes());
        }

        // parent key goes down
        let page_hdr = self.pager.fetch(page_id).header()?;
        let mut page_cells = self.pager.fetch(page_id).get_cells()?;

        let new_first_cell = if page_hdr.page_ty == PageKind::Leaf {
            Cell {
                key: parent_key,
                c_ptr: None,
            }
        } else {
            Cell {
                key: parent_key,
                c_ptr: Some(sibling_rightmost),
            }
        };
        page_cells.insert(0, new_first_cell);

        self.pager.fetch(page_id).reset_and_fill(
            page_id,
            page_hdr.page_ty,
            page_hdr.rightmost_ptr,
            &page_cells,
        )?;

        // sibling cell removal
        if page_hdr.page_ty == PageKind::Leaf {
            self.pager
                .fetch(left_id)
                .remove_cell(left_cells.len() - 1)?;
        } else {
            let mut left_page = self.pager.fetch(left_id);
            left_page.set_rightmost_ptr(sibling_last_ptr.unwrap())?;
            drop(left_page);
            self.pager
                .fetch(left_id)
                .remove_cell(left_cells.len() - 1)?;
        }

        Ok(())
    }

    fn borrow_from_right(
        &mut self,
        page_id: u32,
        right_id: u32,
        parent_id: u32,
        parent_key_idx: usize,
    ) -> Result<(), Box<dyn Error>> {
        let parent_key = self.pager.fetch(parent_id).get_cells()?[parent_key_idx].key;
        let right_cells = self.pager.fetch(right_id).get_cells()?;
        let right_first_cell = &right_cells[0];
        let sibling_first_key = right_first_cell.key;
        let sibling_first_ptr = right_first_cell.c_ptr;

        // sibling first key goes up
        {
            let mut parent_page = self.pager.fetch(parent_id);
            let slot = parent_page.slot(parent_key_idx as u16);
            let start = slot.cell_offset as usize;
            parent_page.data[start..start + 8].copy_from_slice(&sibling_first_key.to_le_bytes());
        }

        // parent key goes down
        let page_hdr = self.pager.fetch(page_id).header()?;
        let mut page_cells = self.pager.fetch(page_id).get_cells()?;

        let (new_last_cell, new_rightmost) = if page_hdr.page_ty == PageKind::Leaf {
            (
                Cell {
                    key: parent_key,
                    c_ptr: None,
                },
                0,
            )
        } else {
            (
                Cell {
                    key: parent_key,
                    c_ptr: Some(page_hdr.rightmost_ptr),
                },
                sibling_first_ptr.unwrap(),
            )
        };
        page_cells.push(new_last_cell);

        self.pager.fetch(page_id).reset_and_fill(
            page_id,
            page_hdr.page_ty,
            if page_hdr.page_ty == PageKind::Leaf {
                0
            } else {
                new_rightmost
            },
            &page_cells,
        )?;

        // sibling first cell removal
        self.pager.fetch(right_id).remove_cell(0)?;

        Ok(())
    }

    fn merge_nodes(
        &mut self,
        left_id: u32,
        right_id: u32,
        parent_id: u32,
        parent_key_idx: usize,
    ) -> Result<(), Box<dyn Error>> {
        let parent_key = self.pager.fetch(parent_id).get_cells()?[parent_key_idx].key;
        let left_hdr = self.pager.fetch(left_id).header()?;
        let right_hdr = self.pager.fetch(right_id).header()?;

        let mut left_cells = self.pager.fetch(left_id).get_cells()?;
        let right_cells = self.pager.fetch(right_id).get_cells()?;

        // parent key merges to left
        if left_hdr.page_ty == PageKind::Leaf {
            left_cells.push(Cell {
                key: parent_key,
                c_ptr: None,
            });
        } else {
            left_cells.push(Cell {
                key: parent_key,
                c_ptr: Some(left_hdr.rightmost_ptr),
            });
        }

        left_cells.extend(right_cells);

        let new_left_rightmost = if left_hdr.page_ty == PageKind::Leaf {
            0
        } else {
            right_hdr.rightmost_ptr
        };

        self.pager.fetch(left_id).reset_and_fill(
            left_id,
            left_hdr.page_ty,
            new_left_rightmost,
            &left_cells,
        )?;

        // fix the parent pointers
        let parent_hdr = self.pager.fetch(parent_id).header()?;
        if parent_hdr.rightmost_ptr == right_id {
            self.pager.fetch(parent_id).set_rightmost_ptr(left_id)?;
        } else {
            self.pager
                .fetch(parent_id)
                .set_child_ptr_at(parent_key_idx + 1, left_id)?;
        }

        // drop parent separating cell
        self.pager.fetch(parent_id).remove_cell(parent_key_idx)?;

        // kill right sibling
        self.pager.remove_entry(right_id);

        Ok(())
    }
}

/// Layout will be like: \[header\]—\[p1\]—\[p2\]—\[free_space\]—\[cell2\]—\[cell2\]
/// the actual page mapping 1:1 to the Node
pub struct Page {
    data: [u8; PAGE_SIZE], // LE
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq)]
enum PageKind {
    /// root node bruh
    Root = 0,
    /// This signify nodes that hold separator key and a pointer to the page between two neighboring pointers, _*Key cells*_
    Internal = 1,
    /// This signify nodes that hold the actual value, _*Key-value cells*_
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
    rightmost_ptr: u32, // don't read when PageKind::Leaf, Internal node are gauranteed to have a rightmost ptr
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
        buf[12..16].copy_from_slice(&self.rightmost_ptr.to_le_bytes());
    }

    pub fn deserialize(buf: &[u8]) -> Option<Self> {
        Some(PageHeader {
            id: u32::from_le_bytes(buf[0..4].try_into().ok()?),
            page_ty: PageKind::from_u8(buf[4])?,
            free_start: u16::from_le_bytes(buf[5..7].try_into().ok()?),
            free_end: u16::from_le_bytes(buf[7..9].try_into().ok()?),
            total_size: u16::from_le_bytes(buf[9..11].try_into().ok()?),
            flags: u8::from_le(buf[11]),
            rightmost_ptr: u32::from_le_bytes(buf[12..16].try_into().ok()?),
        })
    }
}

/// starting offset (cell_offset) → \[Cell\] ← ending offset, 8 bytes
struct CellPointer {
    cell_offset: u16,
    cell_size: u16,
}

/// For `Internal` & `Root` Cells the cells will be structures like \[key: i64 c_ptr: u32\]; 8 + 4
/// For `Leaf` node the cell will only structured like \[key: i64\]; 8
struct Cell {
    // TODO: make keys a generic and pre-compute the key size, but i think should be added as a function
    // argument so there's not redundant calls. note: we also gotta keep the struct (if key is a struct) as a C repr
    pub key: i64,       // 8 bytes
    c_ptr: Option<u32>, // None for PageKind::Leaf, 8 bytes
}

impl Page {
    /// Inserts a new cell and returns the number of keys in currently in the page
    pub fn add_cell(&mut self, c_data: &[u8]) -> Result<usize, Box<dyn Error>> {
        let cell_size = c_data.len();

        let mut hdr = self.header()?;

        let p_offset = hdr.free_start;
        let c_offset = hdr.free_end;

        let end = c_offset as usize;

        // NOTE: overflow page not implemented yet, for now just panic
        // must leave room for the new 4-byte slot entry too, hence + 4
        let start = end
            .checked_sub(cell_size)
            .filter(|&s| s >= p_offset as usize + 4)
            .ok_or("OH NO! Page overflow! can't insert >w<")?;

        self.data[start..end].clone_from_slice(c_data);

        // NOTE: for cell pointer layout im thinking prolly just a 4 byte long sequence(cell_offset: u16, cell_size: u16) after header
        let cell = CellPointer {
            cell_offset: start as u16,
            cell_size: cell_size as u16,
        };

        let new_key = i64::from_le_bytes(c_data[0..8].try_into().unwrap());

        // NOTE: this assumption will probably be wrong in the future as the slot array may
        // contains dead cell pointers...
        let n_slots = ((p_offset as usize - HEADER_SIZE) / 4) as u16;

        let mut lo = 0u16;
        let mut hi = n_slots;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let mid_slot = self.slot(mid);
            let mid_key = i64::from_le_bytes(
                self.data[mid_slot.cell_offset as usize..mid_slot.cell_offset as usize + 8]
                    .try_into()
                    .unwrap(),
            );
            if mid_key < new_key {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let insert_idx = lo as usize;

        // shift every slot at/after insert_idx right by 4 bytes to make room
        let slot_area_start = HEADER_SIZE + insert_idx * 4;
        let slot_area_end = HEADER_SIZE + n_slots as usize * 4;
        self.data
            .copy_within(slot_area_start..slot_area_end, slot_area_start + 4);

        // write new slot into the gap
        self.data[slot_area_start..slot_area_start + 2]
            .copy_from_slice(&cell.cell_offset.to_le_bytes());
        self.data[slot_area_start + 2..slot_area_start + 4]
            .copy_from_slice(&cell.cell_size.to_le_bytes());

        hdr.free_start += 4;
        hdr.free_end = start as u16;
        hdr.serialize(&mut self.data[..HEADER_SIZE]);

        Ok(n_slots as usize + 1)
    }

    fn get_cells(&self) -> Result<Vec<Cell>, Box<dyn Error>> {
        let p_hdr = self.header()?;
        let range = ((p_hdr.free_start as u32 - HEADER_SIZE as u32) / 4) as u16; // 4 bytes per ptr

        // TODO: update the method for accessing the page raw bytes from the file
        let mut cells = Vec::with_capacity(range as usize);

        match p_hdr.page_ty {
            PageKind::Leaf => {
                for i in 0..range {
                    let slot = self.slot(i);
                    let start = slot.cell_offset as usize;
                    let key = i64::from_le_bytes(self.data[start..start + 8].try_into().unwrap());
                    cells.push(Cell { key, c_ptr: None });
                }
            }
            _ => {
                for i in 0..range {
                    let slot = self.slot(i);
                    let start = slot.cell_offset as usize;
                    let key = i64::from_le_bytes(self.data[start..start + 8].try_into().unwrap());
                    let c_ptr =
                        u32::from_le_bytes(self.data[start + 8..start + 12].try_into().unwrap());
                    cells.push(Cell {
                        key,
                        c_ptr: Some(c_ptr),
                    });
                }
            }
        }

        Ok(cells)
    }

    fn reset_and_fill(
        &mut self,
        id: u32,
        page_ty: PageKind,
        rightmost_ptr: u32,
        cells: &[Cell],
    ) -> Result<(), Box<dyn Error>> {
        let hdr = PageHeader {
            id,
            total_size: PAGE_SIZE as u16,
            free_start: HEADER_SIZE as u16,
            free_end: PAGE_SIZE as u16,
            rightmost_ptr,
            page_ty,
            flags: 0,
        };
        hdr.serialize(&mut self.data[0..HEADER_SIZE]);

        for cell in cells {
            let mut buf = Vec::with_capacity(12);
            buf.extend_from_slice(&cell.key.to_le_bytes());
            if let Some(c_ptr) = cell.c_ptr {
                buf.extend_from_slice(&c_ptr.to_le_bytes());
            }
            self.add_cell(&buf)?;
        }

        Ok(())
    }

    fn set_rightmost_ptr(&mut self, ptr: u32) -> Result<(), Box<dyn Error>> {
        let mut hdr = self.header()?;
        hdr.rightmost_ptr = ptr;
        hdr.serialize(&mut self.data[0..HEADER_SIZE]);
        Ok(())
    }

    fn set_child_ptr_at(&mut self, idx: usize, new_ptr: u32) -> Result<(), Box<dyn Error>> {
        let slot = self.slot(idx as u16);
        let start = slot.cell_offset as usize;
        self.data[start + 8..start + 12].copy_from_slice(&new_ptr.to_le_bytes());
        Ok(())
    }

    fn set_page_ty(&mut self, ty: PageKind) -> Result<(), Box<dyn Error>> {
        let mut hdr = self.header()?;
        hdr.page_ty = ty;

        hdr.serialize(&mut self.data[0..HEADER_SIZE]);
        Ok(())
    }

    pub fn remove_cell(&mut self, idx: usize) -> Result<Page, Box<dyn Error>> {
        let hdr = self.header()?;
        let mut cells = self.get_cells()?;
        if idx >= cells.len() {
            return Err("index out of bounds".into());
        }
        cells.remove(idx);
        self.reset_and_fill(hdr.id, hdr.page_ty, hdr.rightmost_ptr, &cells)?;
        Ok(Page { data: self.data })
    }

    fn num_slots(&mut self) -> u16 {
        let hdr = self.header().unwrap();
        (hdr.free_start - HEADER_SIZE as u16) / SLOT_SIZE as u16
    }

    /// Returns the CellPointer for index i in the page
    fn slot(&self, i: u16) -> CellPointer {
        let off = HEADER_SIZE + i as usize * SLOT_SIZE;

        CellPointer {
            cell_offset: u16::from_le_bytes(self.data[off..off + 2].try_into().unwrap()),
            cell_size: u16::from_le_bytes(self.data[off + 2..off + 4].try_into().unwrap()),
        }
    }

    // TODO: for deletion we will later have an availablity list so we can avoid unreachable cells.

    fn cell(&self, i: u16) -> &[u8] {
        let s = self.slot(i);
        let start = s.cell_offset as usize;
        &self.data[start..start + s.cell_size as usize]
    }

    fn header(&self) -> Result<PageHeader, Box<dyn Error>> {
        PageHeader::deserialize(&self.data[..HEADER_SIZE])
            .ok_or("Couldn't deserialize the header".into())
    }
}

pub struct Pager {
    file: std::fs::File,
    frames: RefCell<HashMap<u32, Page>>, // TODO: add eviction poilicies
    path_buf: std::path::PathBuf,
    next_id: u32,
}

impl Pager {
    pub fn flush_all(&mut self) -> Result<(), Box<dyn Error>> {
        let mut ids = Vec::new();
        for id in self.frames.try_borrow_mut().unwrap().keys().copied() {
            ids.push(id);
        }

        for id in ids {
            self.flush(id)?;
        }
        Ok(())
    }

    pub fn flush(&mut self, id: u32) -> Result<(), Box<dyn Error>> {
        if let Some(page) = self.frames.borrow_mut().remove(&id) {
            let offset = Self::page_offset(id);
            self.file.write_all_at(&page.data, offset)?;
        }
        Ok(())
    }

    fn fetch(&self, id: u32) -> RefMut<'_, Page> {
        if !self.frames.borrow().contains_key(&id) {
            let mut buf = [0u8; PAGE_SIZE];
            self.file
                .read_exact_at(&mut buf, Self::page_offset(id))
                .unwrap();
            self.frames.borrow_mut().insert(id, Page { data: buf });
        }

        RefMut::map(self.frames.borrow_mut(), |frames| {
            frames.get_mut(&id).unwrap()
        })
    }

    fn allocate(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.frames.get_mut().insert(
            id,
            Page {
                data: [0u8; PAGE_SIZE],
            },
        );
        id
    }

    fn remove_entry(&mut self, id: u32) {
        self.frames.borrow_mut().remove(&id);
    }

    fn page_offset(id: u32) -> u64 {
        id as u64 * PAGE_SIZE as u64
    }
}
