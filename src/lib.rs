mod trees;

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::trees::{avl::AVLTree, bst::BinarySearchTree, btree::Btree};
    // test bst with `cargo test bst`
    fn bst_sample_tree() -> BinarySearchTree {
        //        50
        //      /    \
        //    30      70
        //   /  \    /  \
        // 20   40  60   80
        let mut tree = BinarySearchTree::new(50, None, None);
        for v in [30, 70, 20, 40, 60, 80] {
            tree.insert(v);
        }
        tree
    }

    #[test]
    fn bst_insert_and_exists() {
        let tree = bst_sample_tree();
        assert!(tree.exists(50));
        assert!(tree.exists(20));
        assert!(tree.exists(80));
        assert!(!tree.exists(99));
        assert!(!tree.exists(-5));
    }

    #[test]
    fn bst_insert_duplicate_goes_right() {
        let mut tree = BinarySearchTree::new(50, None, None);
        tree.insert(50);
        assert!(tree.right.is_some());
        assert_eq!(tree.right.unwrap().val, 50);
    }

    #[test]
    fn bst_delete_leaf_node() {
        let mut tree = bst_sample_tree();
        tree.delete(20);
        assert!(!tree.exists(20));
        assert!(tree.exists(30));
        assert!(tree.exists(40));
    }

    #[test]
    fn bst_delete_node_with_one_child() {
        let mut tree = BinarySearchTree::new(50, None, None);
        for v in [30, 20] {
            tree.insert(v);
        }
        tree.delete(30);
        assert!(!tree.exists(30));
        assert!(tree.exists(20));
    }

    #[test]
    fn bst_delete_node_with_two_children() {
        let mut tree = bst_sample_tree();
        tree.delete(30);
        assert!(!tree.exists(30));
        assert!(tree.exists(20));
        assert!(tree.exists(40));
        assert!(tree.exists(50));
        assert!(tree.exists(70));
    }

    #[test]
    fn bst_delete_root_with_two_children() {
        let mut tree = bst_sample_tree();
        tree.delete(50);
        assert_eq!(tree.val, 50);
    }

    #[test]
    fn bst_delete_nonexistent_value_does_not_panic() {
        let mut tree = bst_sample_tree();
        tree.delete(999);
        assert!(tree.exists(50));
    }

    #[test]
    fn bst_distance_to_root_is_zero() {
        let tree = bst_sample_tree();
        assert_eq!(tree.distance(50), 0);
    }

    #[test]
    fn bst_distance_to_child() {
        let tree = bst_sample_tree();
        assert_eq!(tree.distance(30), 1);
        assert_eq!(tree.distance(20), 2);
    }

    #[test]
    #[should_panic]
    fn bst_distance_to_missing_value_panics() {
        let tree = bst_sample_tree();
        tree.distance(999);
    }

    fn avl_sample_tree() -> AVLTree {
        let mut tree = AVLTree::new(50, None, None, 0);
        for v in [30, 70, 20, 40, 60, 80] {
            tree.balanced_insert(v);
        }
        tree
    }

    #[test]
    fn avl_insert_and_exists() {
        let tree = avl_sample_tree();
        assert!(tree.exists(50));
        assert!(tree.exists(20));
        assert!(tree.exists(80));
        assert!(!tree.exists(99));
        assert!(!tree.exists(-5));
    }

    #[test]
    fn avl_insert_duplicate_ignored() {
        let mut tree = AVLTree::new(50, None, None, 0);
        tree.balanced_insert(50);
        assert!(tree.left.is_none());
        assert!(tree.right.is_none());
    }

    #[test]
    fn avl_delete_leaf_node() {
        let mut tree = avl_sample_tree();
        tree.balanced_delete(20);
        assert!(!tree.exists(20));
        assert!(tree.exists(30));
        assert!(tree.exists(40));
    }

    #[test]
    fn avl_delete_node_with_one_child() {
        let mut tree = avl_sample_tree();
        tree.balanced_delete(60);
        tree.balanced_delete(70);
        assert!(!tree.exists(70));
        assert!(tree.exists(80));
    }

    #[test]
    fn avl_delete_node_with_two_children() {
        let mut tree = avl_sample_tree();
        tree.balanced_delete(30);
        assert!(!tree.exists(30));
        assert!(tree.exists(20));
        assert!(tree.exists(40));
        assert!(tree.exists(50));
        assert!(tree.exists(70));
    }

    #[test]
    fn avl_delete_root_is_not_supported() {
        let mut tree = avl_sample_tree();
        tree.balanced_delete(50);
        assert_eq!(tree.val, 50);
    }

    #[test]
    fn avl_delete_nonexistent_value_does_not_panic() {
        let mut tree = avl_sample_tree();
        tree.balanced_delete(999);
        assert!(tree.exists(50));
    }

    #[test]
    fn avl_distance_to_root_is_zero() {
        let tree = avl_sample_tree();
        assert_eq!(tree.distance(50), 0);
    }

    #[test]
    fn avl_distance_to_child() {
        let tree = avl_sample_tree();
        assert_eq!(tree.distance(30), 1);
        assert_eq!(tree.distance(20), 2);
    }

    #[test]
    #[should_panic]
    fn avl_distance_to_missing_value_panics() {
        let tree = avl_sample_tree();
        tree.distance(999);
    }

    fn btree_sample_tree() -> Result<Btree, Box<dyn Error>> {
        let mut btree = Btree::new(20, "./new.db".into())?;
        btree.insert(5)?;
        btree.insert(7)?;
        btree.insert(3)?;
        btree.insert(1)?;
        btree.insert(9)?;
        btree.insert(6)?;
        Ok(btree)
    }

    #[test]
    fn btree_check_search() {
        let btree = btree_sample_tree().unwrap();
        assert!(btree.search(6).unwrap());
    }
}
