mod trees;

struct Cell {
    key: i64,
    c_ptr: Option<u32>, // None for PageKind::Leaf
}

fn main() {
    println!("{}", std::mem::size_of::<Cell>());
}
