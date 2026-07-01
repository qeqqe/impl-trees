#![allow(dead_code)]

#[derive(Default)]
pub struct BinarySearchTree {
    pub val: i64,
    pub left: Option<Box<BinarySearchTree>>,
    pub right: Option<Box<BinarySearchTree>>,
}

impl BinarySearchTree {
    pub fn new(
        val: i64,
        left: Option<Box<BinarySearchTree>>,
        right: Option<Box<BinarySearchTree>>,
    ) -> Self {
        Self { val, left, right }
    }

    // this implementation will not have balanced tree, that's for the AVL Tree
    pub fn insert(&mut self, val: i64) {
        if self.val > val {
            match self.left {
                Some(ref mut left) => left.insert(val),
                None => self.left = Some(Box::new(BinarySearchTree::new(val, None, None))),
            }
        } else {
            match self.right {
                Some(ref mut right) => right.insert(val),
                None => self.right = Some(Box::new(BinarySearchTree::new(val, None, None))),
            }
        }
    }

    pub fn delete(&mut self, val: i64) {
        if self.val > val {
            if let Some(mut left) = self.left.take() {
                if left.val == val {
                    let ll = left.left.take();
                    let lr = left.right.take();

                    self.left = lr;
                    // after this we need to append the ll node to the leftmost end of the lr node
                    if Option::is_some(&self.left) {
                        if let Some(ll_node) = ll {
                            self.left.as_mut().unwrap().insert_left_most(ll_node);
                        }
                    } else {
                        // no lr to inherit so ll node becomes the replacement
                        self.left = ll;
                    }
                    drop(left);
                } else {
                    left.delete(val);
                    self.left = Some(left);
                }
            } else {
                eprintln!("no node found with value {}", val)
            }
        } else {
            if let Some(mut right) = self.right.take() {
                if right.val == val {
                    let rl = right.left.take();
                    let rr = right.right.take();

                    self.right = rr;

                    // append rl to the left most end of rr
                    if Option::is_some(&self.right) {
                        if let Some(rl_node) = rl {
                            self.right.as_mut().unwrap().insert_left_most(rl_node);
                        }
                    } else {
                        self.right = rl;
                    }
                    drop(right);
                } else {
                    right.delete(val);
                    self.right = Some(right);
                }
            } else {
                eprintln!("no node found with value {}", val)
            }
        }
    }

    pub fn exists(&self, val: i64) -> bool {
        if self.val > val {
            match self.left {
                Some(ref left) => left.exists(val),
                None => false,
            }
        } else if self.val < val {
            match self.right {
                Some(ref right) => right.exists(val),
                None => false,
            }
        } else {
            true
        }
    }

    pub fn distance(&self, val: i64) -> usize {
        let mut count: usize = 0;
        self.recurse_distance(val, &mut count);
        count
    }

    fn recurse_distance(&self, val: i64, count: &mut usize) {
        if self.val > val {
            match self.left {
                Some(ref left) => {
                    *count = count.wrapping_add(1);
                    left.recurse_distance(val, count)
                }
                None => panic!(),
            }
        } else if self.val < val {
            match self.right {
                Some(ref right) => {
                    *count = count.wrapping_add(1);
                    right.recurse_distance(val, count)
                }
                None => panic!(),
            }
        }
    }

    fn insert_left_most(&mut self, node: Box<BinarySearchTree>) {
        match self.left {
            Some(ref mut left) => left.insert_left_most(node),
            None => self.left = Some(node),
        }
    }
}

