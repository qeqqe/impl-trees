#![allow(dead_code)]

#[derive(Debug, Clone)]
pub struct AVLTree {
    pub val: i32,
    pub left: Option<Box<AVLTree>>,
    pub right: Option<Box<AVLTree>>,
    bal_factor: isize,
}

impl AVLTree {
    pub fn new(
        val: i32,
        left: Option<Box<AVLTree>>,
        right: Option<Box<AVLTree>>,
        bal_factor: isize,
    ) -> Self {
        Self {
            val,
            left,
            right,
            bal_factor,
        }
    }

    pub fn balanced_insert(&mut self, val: i32) -> bool {
        if val < self.val {
            let height_increased = match self.left {
                Some(ref mut left) => left.balanced_insert(val),
                None => {
                    self.left = Some(Box::new(AVLTree::new(val, None, None, 0)));
                    true
                }
            };

            if height_increased {
                self.bal_factor += 1;
                if self.bal_factor > 1 {
                    // imbalance due to left heaviness, needs child's bal_factor checking
                    if self.left.as_ref().unwrap().bal_factor < 0 {
                        self.left.as_mut().unwrap().rotate_left(); // LR
                    }
                    self.rotate_right(); // LL
                    return false;
                }
                return self.bal_factor != 0;
            }
        } else if val > self.val {
            let height_increased = match self.right {
                Some(ref mut right) => right.balanced_insert(val),
                None => {
                    self.right = Some(Box::new(AVLTree::new(val, None, None, 0)));
                    true
                }
            };

            if height_increased {
                self.bal_factor -= 1;
                if self.bal_factor < -1 {
                    // imbalance due to right heaviness, needs child's bal_factor checking
                    if self.right.as_ref().unwrap().bal_factor > 0 {
                        self.right.as_mut().unwrap().rotate_right(); // RL
                    }
                    self.rotate_left(); // RR (or final step of RL)
                    return false;
                }
                return self.bal_factor != 0;
            }
        } else {
            eprintln!("repeated elements not allowed!");
        }
        false
    }

    pub fn balanced_delete(&mut self, val: i32) -> bool {
        if val < self.val {
            if let Some(mut left) = self.left.take() {
                let shrunk = if left.val == val {
                    let mut slot = Some(left);
                    let s = Self::remove_node(&mut slot);
                    self.left = slot;
                    s
                } else {
                    let s = left.balanced_delete(val);
                    self.left = Some(left);
                    s
                };
                if shrunk {
                    self.bal_factor -= 1; // left side got shorter
                    return self.rebalance_left_shrunk();
                }
            } else {
                eprintln!("no node found with value {}", val);
            }
        } else if val > self.val {
            if let Some(mut right) = self.right.take() {
                let shrunk = if right.val == val {
                    let mut slot = Some(right);
                    let s = Self::remove_node(&mut slot);
                    self.right = slot;
                    s
                } else {
                    let s = right.balanced_delete(val);
                    self.right = Some(right);
                    s
                };
                if shrunk {
                    self.bal_factor += 1; // right side got shorter
                    return self.rebalance_right_shrunk();
                }
            } else {
                eprintln!("no node found with value {}", val);
            }
        } else {
            eprintln!("cannot delete a node from inside itself, remove it from the parent :/"); // no root deletion support for now
        }
        false
    }

    pub fn exists(&self, val: i32) -> bool {
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

    pub fn distance(&self, val: i32) -> usize {
        let mut count: usize = 0;
        self.recurse_distance(val, &mut count);
        count
    }

    fn recurse_distance(&self, val: i32, count: &mut usize) {
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

    // here we remove node that sits in `slot` (be it 0/1/2 child/s) and returns if the height
    // at `slot` shrank. In case of 2 childs we never delete the node's own box, we instead just
    // copy the in-order successor's val up and delete the succsesor instead cus the successor
    // is guaranteed to have at most one child.
    fn remove_node(slot: &mut Option<Box<AVLTree>>) -> bool {
        let mut node = slot.take().unwrap();
        let shrunk = match (node.left.take(), node.right.take()) {
            (None, None) => {
                *slot = None;
                return true;
            }
            (Some(l), None) => {
                *slot = Some(l);
                return true;
            }
            (None, Some(r)) => {
                *slot = Some(r);
                return true;
            }
            (Some(l), Some(r)) => {
                node.left = Some(l);
                let mut right_slot = Some(r);
                let (succ_val, right_shrunk) = Self::pull_leftmost(&mut right_slot);
                node.right = right_slot;
                node.val = succ_val;
                if right_shrunk {
                    node.bal_factor += 1;
                    node.rebalance_right_shrunk()
                } else {
                    false
                }
            }
        };
        *slot = Some(node);
        shrunk
    }

    // here we descend left as far as possible from `slot`, removes that node, and returns
    // (its val, whether `slot`'s height shrank), its possibly that the leftmost node still have a
    // right child (never a left one), so it gets promoted in its place
    fn pull_leftmost(slot: &mut Option<Box<AVLTree>>) -> (i32, bool) {
        let mut node = slot.take().unwrap();
        if node.left.is_none() {
            let val = node.val;
            *slot = node.right.take();
            (val, true)
        } else {
            let mut left_slot = node.left.take();
            let (val, left_shrunk) = Self::pull_leftmost(&mut left_slot);
            node.left = left_slot;
            let shrunk = if left_shrunk {
                node.bal_factor -= 1;
                node.rebalance_left_shrunk()
            } else {
                false
            };
            *slot = Some(node);
            (val, shrunk)
        }
    }

    // call right after self.bal_factor -= 1 cus the left side lost a level.
    // returns whether self's own height decreased, so the caller knows whether
    // to keep propagating the shrink upward
    fn rebalance_left_shrunk(&mut self) -> bool {
        match self.bal_factor {
            -1 => false, // right was already one taller, still tolerated
            0 => true,   // was balanced, now left is shorter, overall height dropped
            -2 => {
                let right_bf = self.right.as_ref().unwrap().bal_factor;
                if right_bf <= 0 {
                    self.rotate_left(); // RR
                    right_bf != 0
                } else {
                    self.right.as_mut().unwrap().rotate_right(); // RL, but fix child first
                    self.rotate_left();
                    true // double rotation always drops height by one
                }
            }
            _ => unreachable!("bal_factor left -2..=2, insert/delete bug"),
        }
    }

    // mirror rebalance_left_shrunk for the right side shrink
    fn rebalance_right_shrunk(&mut self) -> bool {
        match self.bal_factor {
            1 => false,
            0 => true,
            2 => {
                let left_bf = self.left.as_ref().unwrap().bal_factor;
                if left_bf >= 0 {
                    self.rotate_right(); // LL
                    left_bf != 0
                } else {
                    self.left.as_mut().unwrap().rotate_left(); // LR, but fix child first
                    self.rotate_right();
                    true
                }
            }
            _ => unreachable!("bal_factor left -2..=2, insert/delete bug"),
        }
    }

    fn rotate_right(&mut self) {
        // steps (LL rotation)
        // Suppose the Node X is having an imbalance because node Y is too heavy
        // 1. First append any node right to the node Y TO left of Node X (gauranteed to be
        //    smaller so valid)
        // 2. calculate new BF
        // 3. swap X and Y node
        // 4. append the Y (privously X) to the right of X (priviously Y)
        let mut y = self.left.take().unwrap();
        self.left = y.right.take(); // step 1, this line was missing, was dropping the subtree

        let old_bf_x = self.bal_factor;
        let old_bf_y = y.bal_factor;

        let new_bf_x = old_bf_x - 1 - std::cmp::max(old_bf_y, 0); // - mx(OBFY, 0) bf will dec as it's losing level from left
        let new_bf_y = old_bf_y - 1 + std::cmp::min(new_bf_x, 0); // gaining level

        self.bal_factor = new_bf_x;
        y.bal_factor = new_bf_y;

        std::mem::swap(self, &mut y);

        self.right = Some(y);
    }

    fn rotate_left(&mut self) {
        // the RR rotation
        // steps
        // 1. First append any node left to the node Y TO right of Node X (gauranteed to be
        //    bigger so valid)
        // 2. calculate new BF
        // 3. swap X and Y node
        // 4. append the Y (privously X) to the left of X (priviously Y)
        let mut y = self.right.take().unwrap();
        self.right = y.left.take();

        let old_bf_x = self.bal_factor;
        let old_bf_y = y.bal_factor;

        let new_bf_x = old_bf_x + 1 - std::cmp::min(old_bf_y, 0);
        let new_bf_y = old_bf_y + 1 + std::cmp::max(0, new_bf_x);

        self.bal_factor = new_bf_x;
        y.bal_factor = new_bf_y;

        std::mem::swap(self, &mut y);
        self.left = Some(y);
    }
}
