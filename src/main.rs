use crate::trees::btree::Btree;

mod trees;
fn main() {
    let mut btree = Btree::new(20, "./new".into()).unwrap();
    btree.insert(9).unwrap();
    btree.insert(6).unwrap();
    btree.insert(7).unwrap();
    btree.insert(21).unwrap();
    btree.insert(32).unwrap();
    btree.insert(43).unwrap();
    btree.insert(12).unwrap();
    btree.insert(3).unwrap();

    println!("{}", btree);
}
