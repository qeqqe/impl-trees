mod trees;

struct Cell {
    // TODO: make keys a generic and pre-compute the key size, but i think should be added as a function
    // argument so there's not redundant calls. note: we also gotta keep the struct (if key is a struct) as a C repr
    key: i64,
    c_ptr: Option<u32>, // None for PageKind::Leaf
}

fn main() {
    println!("{}", std::mem::size_of::<Cell>());
}
