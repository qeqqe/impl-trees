use crate::trees::btree::Btree;

mod trees;
fn main() {
    let mut btree = Btree::new(20, "./new".into()).unwrap();
    btree.insert(5).unwrap();
    btree.insert(7).unwrap();
    btree.insert(3).unwrap();
    btree.insert(1).unwrap();
    btree.insert(9).unwrap();
    btree.insert(6).unwrap();

    println!("{:#?}", btree.root_id);
}
