use msp::{bytes_to_hex_str, hex_str_to_bytes};
use std::clone;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::{
    borrow::BorrowMut,
    collections::LinkedList,
    sync::{Arc, Mutex},
};
use utils::general::{hash_bytes_to_string, hash_crypto};

struct Node {
    parent: Vec<u8>,
    children: [Vec<u8>; 2],
    hash: Vec<u8>,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.parent == other.parent && self.children == other.children && self.hash == other.hash
    }
}

impl Clone for Node {
    fn clone_from(&mut self, source: &Self) {
        *self = source.clone()
    }

    fn clone(&self) -> Self {
        Self {
            parent: self.parent.clone(),
            children: [self.children[0].clone(), self.children[1].clone()],
            hash: self.hash.clone(),
        }
    }
}

impl Node {
    pub fn new() -> Node {
        Node {
            parent: Vec::new(),
            children: [Vec::new(), Vec::new()],
            hash: Vec::new(),
        }
    }

    pub fn set_hash(&mut self, hash: Vec<u8>) {
        self.hash.clone_from(&hash);
    }

    pub fn get_parent(&self) -> Vec<u8> {
        self.parent.clone()
    }

    pub fn set_parent(&mut self, parent: Vec<u8>) {
        self.parent.clone_from(&parent);
    }

    pub fn set_children(&mut self, children_l: Vec<u8>, children_r: Vec<u8>) {
        self.children[0].clone_from(&children_l);
        self.children[1].clone_from(&children_r);
    }

    pub fn get_children(&self, index: usize) -> Vec<u8> {
        if index <= 1 {
            return self.children[index].clone();
        } else {
            return Vec::new();
        }
    }

    pub fn get_hash(&self) -> Vec<u8> {
        self.hash.clone()
    }
}

pub struct MerkleProofHash {
    hash: Vec<u8>,
    direction: Branch,
}

#[derive(PartialEq)]
pub enum Branch {
    Left,
    Right,
    OldRoot,
}

impl MerkleProofHash {
    pub fn new(hash: Vec<u8>, direction: Branch) -> MerkleProofHash {
        MerkleProofHash {
            hash: hash,
            direction: direction,
        }
    }
}

struct Tree {
    base: Vec<Vec<Node>>,
    merkle_root: Vec<u8>,
    node_list: HashMap<Vec<u8>, Node>,
}

impl Tree {
    pub fn new() -> Tree {
        Tree {
            base: Vec::new(),
            merkle_root: Vec::new(),
            node_list: HashMap::new(),
        }
    }

    pub fn find_node(&self, hash: Vec<u8>) -> Node {
        for vec_in_nodes in &self.base {
            for node in vec_in_nodes {
                if node.get_hash() == hash {
                    return node.clone();
                }
            }
        }
        return Node::new();
    }

    pub fn println_nodes(&self) {
        for vec_in_nodes in &self.base {
            for node in vec_in_nodes {
                println!(
                    "node hsah is:{:?}",
                    hash_bytes_to_string(node.get_hash().as_slice())
                );
            }
        }
    }

    pub fn get_sibling(&self, node: &Node) -> Vec<u8> {
        let parent = self.find_node(node.get_parent());
        let children_0 = self.find_node(parent.get_children(0));
        if node.eq(&children_0) {
            return parent.get_children(1);
        } else {
            return parent.get_children(0);
        }
    }

    pub fn check_dir(&self, node: &Node) -> bool {
        let parent = self.find_node(node.get_parent());
        let children_0 = self.find_node(parent.get_children(0));
        if node.eq(&children_0) {
            return false;
        } else {
            return true;
        }
    }

    pub fn make_binary(&mut self) -> usize {
        let mut vect_size = 0;
        if let Some(last) = self.base.last_mut() {
            vect_size = last.len();
            if vect_size % 2 != 0 {
                last.push(last[vect_size - 1].clone());
                return vect_size + 1;
            }
        }
        vect_size
    }

    fn build_tree(&mut self) {
        loop {
            let mut new_nodes: Vec<Node> = Vec::new();
            let vect_size = self.make_binary();

            for i in (0..vect_size).step_by(2) {
                let mut new_parent = Node::new();
                if let Some(last) = self.base.last_mut() {
                    let hash =
                        Self::hash_merkle_branches((last[i].get_hash()), (last[i + 1].get_hash()));
                    new_parent.set_hash(hash.clone());
                    last[i].set_parent(hash.clone());
                    last[i + 1].set_parent(hash.clone());
                    new_parent.set_children(last[i].get_hash(), last[i + 1].get_hash());
                    if let Some(last_i) = self.node_list.get_mut(&last[i].get_hash()) {
                        last_i.set_parent(hash.clone());
                    }
                    if let Some(last_i1) = self.node_list.get_mut(&last[i + 1].get_hash()) {
                        last_i1.set_parent(hash.clone());
                    }
                }
                new_nodes.push(new_parent.clone());
                self.node_list
                    .insert(new_parent.clone().get_hash(), new_parent.clone());
            }

            self.base.push(new_nodes);

            if self.base.last().unwrap().len() <= 1 {
                break;
            }
        }

        if let Some(last) = self.base.last() {
            self.merkle_root = last[0].get_hash();
        }
    }

    pub fn build_base_leafs(&mut self, base_leafs: Vec<Vec<u8>>) {
        let mut new_nodes: Vec<Node> = Vec::new();
        for leaf in base_leafs {
            let mut new_node = Node::new();
            new_node.set_hash(leaf.clone());
            new_nodes.push(new_node.clone());
            self.node_list.insert(leaf.clone(), new_node.clone());
        }

        self.base.push(new_nodes);
    }

    pub fn verify(&self, hash: Vec<u8>) -> bool {
        let mut el_node: Node = self.find_node(hash.clone());
        let mut act_hash = hash.clone();
        loop {
            let mut sibling_hash = self.get_sibling(&el_node);
            if self.check_dir(&el_node) == false {
                act_hash.clone_from(&Self::hash_merkle_branches(act_hash.clone(), sibling_hash));
            } else {
                act_hash.clone_from(&Self::hash_merkle_branches(sibling_hash, act_hash.clone()));
            }
            let parent = self.find_node(act_hash.clone());
            el_node.clone_from(&parent);
            if parent.get_parent().len() <= 0 {
                break;
            }
        }

        if act_hash == self.merkle_root {
            return true;
        } else {
            return false;
        }
    }

    pub fn hash_merkle_branches(left: Vec<u8>, right: Vec<u8>) -> Vec<u8> {
        let hash_str = format!(
            "{}{}",
            hash_bytes_to_string(left.as_slice()),
            hash_bytes_to_string(right.as_slice())
        );
        hash_crypto(hash_str.as_bytes().to_vec().as_slice())
    }

    pub fn build_audit_trail(
        &mut self,
        audit_trail: &mut Vec<MerkleProofHash>,
        parent: &mut Vec<u8>,
        child: &mut Vec<u8>,
    ) {
        let mut direction = Branch::OldRoot;
        let mut next_child: Vec<u8> = Vec::new();
        let parent_node = self.find_node(parent.clone());
        let children_node = self.find_node(parent_node.get_children(0));
        let child_node = self.find_node(child.clone());
        if children_node.eq(&child_node) {
            next_child = parent_node.get_children(1);
            direction = Branch::Left;
        } else {
            next_child = parent_node.get_children(0);
            direction = Branch::Right;
        }

        audit_trail.push(MerkleProofHash {
            hash: next_child,
            direction: direction,
        });

        if parent_node.get_parent().len() >= 1 {
            self.build_audit_trail(audit_trail, parent, child);
        }
    }

    pub fn verify_audit(
        &self,
        root_hash: Vec<u8>,
        leaf_hash: Vec<u8>,
        audit_trail: &Vec<MerkleProofHash>,
    ) -> bool {
        if audit_trail.is_empty() {
            return false;
        }

        let mut act_hash = Vec::from(leaf_hash);
        for proof_hash in audit_trail {
            if proof_hash.direction == Branch::Left {
                act_hash = Self::hash_merkle_branches(act_hash, proof_hash.hash.clone());
            } else {
                act_hash = Self::hash_merkle_branches(proof_hash.hash.clone(), act_hash);
            }
        }
        return root_hash == act_hash;
    }

    pub fn audit_proof(&mut self, leaf_hash: &mut Vec<u8>, audit_trail: &mut Vec<MerkleProofHash>) {
        let mut leaf_node: Node = self.find_node(leaf_hash.clone());
        let mut parent_hash = leaf_node.get_parent();
        self.build_audit_trail(audit_trail, &mut parent_hash, leaf_hash);
    }
}

#[test]
fn test_double_tree() {
    let mut tree = Tree::new();
    let base_leafs = vec![
        hash_crypto(&[0]),
        hash_crypto(&[1]),
        hash_crypto(&[2]),
        hash_crypto(&[3]),
        hash_crypto(&[4]),
        hash_crypto(&[5]),
        hash_crypto(&[6]),
        hash_crypto(&[7]),
        hash_crypto(&[8]),
        hash_crypto(&[9]),
        hash_crypto(&[10]),
        hash_crypto(&[11]),
        hash_crypto(&[12]),
        hash_crypto(&[13]),
        hash_crypto(&[14]),
        hash_crypto(&[15]),
    ];
    tree.build_base_leafs(base_leafs.clone());
    tree.build_tree();
    tree.println_nodes();
    for node in base_leafs.clone() {
        let result = tree.verify(node);
        assert_eq!(true, result);
    }
}

#[test]
fn test_single_tree() {
    let mut tree = Tree::new();
    let base_leafs = vec![
        hash_crypto(&[1]),
        hash_crypto(&[2]),
        hash_crypto(&[3]),
        hash_crypto(&[4]),
        hash_crypto(&[5]),
        hash_crypto(&[6]),
        hash_crypto(&[7]),
        hash_crypto(&[8]),
        hash_crypto(&[9]),
        hash_crypto(&[10]),
        hash_crypto(&[11]),
        hash_crypto(&[12]),
        hash_crypto(&[13]),
        hash_crypto(&[14]),
        hash_crypto(&[15]),
    ];
    tree.build_base_leafs(base_leafs.clone());
    tree.build_tree();
    tree.println_nodes();
    for node in base_leafs.clone() {
        let result = tree.verify(node);
        assert_eq!(true, result);
    }
}

#[test]
fn test_audit_tree() {
    let mut tree = Tree::new();
    let base_leafs = vec![
        ("leaf1").as_bytes().to_vec(),
        ("leaf2").as_bytes().to_vec(),
        ("leaf3").as_bytes().to_vec(),
        ("leaf4").as_bytes().to_vec(),
    ];
    tree.build_base_leafs(base_leafs);
    tree.build_tree();

    let hash = Tree::hash_merkle_branches(
        ("5feceb66ffc86f38d952786c6d696c79c2dbc239dd4e91b46729d73a27fb57e9")
            .as_bytes()
            .to_vec(),
        ("6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b")
            .as_bytes()
            .to_vec(),
    );
    let mut audit_trail: Vec<MerkleProofHash> = Vec::new();
    tree.audit_proof(
        &mut ("4a44dc15364204a80fe80e9039455cc1608281820fe2b24f1e5233ade6af1dd5")
            .as_bytes()
            .to_vec(),
        &mut audit_trail,
    );
    let result = tree.verify_audit(
        ("a901f842b0016f1e350d20b751851a7179e26dfbb74b213c7a92d37f3c4fbb6c")
            .as_bytes()
            .to_vec(),
        ("4a44dc15364204a80fe80e9039455cc1608281820fe2b24f1e5233ade6af1dd5")
            .as_bytes()
            .to_vec(),
        &audit_trail,
    );
    assert_eq!(true, result);
}
