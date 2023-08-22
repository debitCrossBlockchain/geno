use msp::{bytes_to_hex_str, hex_str_to_bytes};
use std::ops::DerefMut;
use std::{
    borrow::BorrowMut,
    collections::LinkedList,
    sync::{Arc, Mutex},
};
use utils::general::{hash_bytes_to_string, hash_crypto};

#[derive(Clone)]
struct Node {
    parent: Option<Box<Node>>,
    children: [Option<Box<Node>>; 2],
    hash: Vec<u8>,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.parent == other.parent && self.children == other.children && self.hash == other.hash
    }
}

impl Node {
    pub fn new() -> Node {
        Node {
            parent: None,
            children: [None, None],
            hash: Vec::new(),
        }
    }

    pub fn set_hash(&mut self, hash: Vec<u8>) {
        self.hash.clone_from(&hash);
    }

    pub fn get_parent(&self) -> Option<&Box<Node>> {
        self.parent.as_ref()
    }

    pub fn set_parent(&mut self, parent: Box<Node>) {
        self.parent = Some(parent);
    }

    pub fn set_children(&mut self, children_l: Box<Node>, children_r: Box<Node>) {
        self.children[0] = Some(children_l);
        self.children[1] = Some(children_r);
    }

    pub fn get_sibling(&self) -> Option<&Box<Node>> {
        let parent = self.get_parent();
        match parent {
            Some(p) => {
                if self.eq(p.children[0].as_ref().unwrap()) {
                    return p.children[1].as_ref();
                } else {
                    return p.children[0].as_ref();
                }
            }
            None => None,
        }
    }

    pub fn get_children(&self, index: usize) -> Option<&Box<Node>> {
        if index <= 1 {
            return self.children[index].as_ref();
        } else {
            return None;
        }
    }

    pub fn get_hash(&self) -> Vec<u8> {
        self.hash.clone()
    }

    pub fn check_dir(&self) -> bool {
        let parent = self.get_parent();
        match parent {
            Some(p) => {
                if self.eq(p.children[0].as_ref().unwrap()) {
                    return false;
                } else {
                    return true;
                }
            }
            None => false,
        }
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
    base: Arc<Mutex<Vec<Vec<Box<Node>>>>>,
    merkle_root: Vec<u8>,
    node_list: Arc<Mutex<Vec<Box<Node>>>>,
}

impl Tree {
    pub fn new() -> Tree {
        Tree {
            base: Arc::new(Mutex::new(Vec::new())),
            merkle_root: Vec::new(),
            node_list: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn make_binary(&mut self) -> usize {
        let vect_size = (self.base.lock().unwrap().last().unwrap()).len();
        if vect_size % 2 != 0 {
            (self.base.lock().unwrap().last_mut().unwrap())
                .push(self.base.lock().unwrap().last_mut().unwrap()[vect_size - 1].clone());
            return vect_size + 1;
        }
        vect_size
    }

    fn build_tree(&mut self) {
        loop {
            let mut new_nodes: Vec<Box<Node>> = Vec::new();
            let vect_size = self.make_binary();

            for i in (0..vect_size).step_by(2) {
                let mut new_parent = Box::new(Node::new());
                (self.base.lock().unwrap().last_mut().unwrap()[i].set_parent(new_parent.clone()));
                (self.base.lock().unwrap().last_mut().unwrap()[i + 1]
                    .set_parent(new_parent.clone()));

                let hash = Self::hash_merkle_branches(
                    (self.base.lock().unwrap().last().unwrap()[i].get_hash()),
                    (self.base.lock().unwrap().last().unwrap()[i + 1].get_hash()),
                );
                new_parent.set_hash(hash);

                new_parent.set_children(
                    self.base.lock().unwrap().last().unwrap()[i].clone(),
                    self.base.lock().unwrap().last().unwrap()[i + 1].clone(),
                );

                new_nodes.push(new_parent);
            }

            (self.node_list.lock().unwrap().append(&mut new_nodes));
            (self.base.lock().unwrap().push(new_nodes));

            if (self.base.lock().unwrap().last().unwrap().len()) <= 1 {
                break;
            }
        }

        self.merkle_root = self.base.lock().unwrap().last_mut().unwrap()[0].get_hash();
    }

    pub fn build_base_leafs(&mut self, base_leafs: Vec<Vec<u8>>) {
        let mut new_nodes: Vec<Box<Node>> = Vec::new();
        for leaf in base_leafs {
            let mut new_node = Box::new(Node::new());
            new_node.set_hash(leaf.clone());
            new_nodes.push(new_node);
        }

        self.node_list.lock().unwrap().append(&mut new_nodes);
    }

    pub fn verify(&self, hash: Vec<u8>) -> bool {
        let mut el_node: Option<Box<Node>> = None;
        let tem_hash = hash.clone();
        let mut act_hash = hash;
        let len = (self.base.lock().unwrap().first().unwrap().len());
        for i in 0..len {
            if (self.base.lock().unwrap().first().unwrap()[i].get_hash()) == tem_hash {
                el_node = Some(self.base.lock().unwrap().first().unwrap()[i].clone());
            }
        }

        if el_node.is_none() {
            return false;
        }

        let mut el_node = el_node.unwrap();
        loop {
            if el_node.check_dir() == false {
                act_hash =
                    Self::hash_merkle_branches(act_hash, el_node.get_sibling().unwrap().get_hash());
            } else {
                act_hash =
                    Self::hash_merkle_branches(el_node.get_sibling().unwrap().get_hash(), act_hash);
            }

            match el_node.get_parent() {
                Some(p) => el_node = p.clone(),
                None => break,
            }
        }

        if tem_hash == self.merkle_root {
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
        hash_crypto(hex_str_to_bytes(hash_str.as_str()).unwrap().as_slice())
    }

    pub fn build_audit_trail(
        &mut self,
        audit_trail: &mut Vec<MerkleProofHash>,
        parent: &mut &Box<Node>,
        child: &mut &Box<Node>,
    ) {
        let mut direction = Branch::OldRoot;
        let mut next_child: Option<Box<Node>> = None;
        if let Some(children) = parent.get_children(0) {
            if children.eq(child) {
                next_child = Some(parent.get_children(1).unwrap().clone());
                direction = Branch::Left;
            } else {
                next_child = Some(parent.get_children(0).unwrap().clone());
                direction = Branch::Right;
            }

            audit_trail.push(MerkleProofHash {
                hash: next_child.unwrap().get_hash(),
                direction: direction,
            });

            if let Some(c) = (&mut child.get_parent()).as_mut() {
                if let Some(p) = c.get_parent().as_mut() {
                    self.build_audit_trail(audit_trail, p, c);
                }
            }
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

        let mut test_hash = Vec::from(leaf_hash);
        for proof_hash in audit_trail {
            if proof_hash.direction == Branch::Left {
                test_hash = Self::hash_merkle_branches(test_hash, proof_hash.hash.clone());
            } else {
                test_hash = Self::hash_merkle_branches(proof_hash.hash.clone(), test_hash);
            }
        }
        return root_hash == test_hash;
    }

    pub fn audit_proof(&mut self, leaf_hash: Vec<u8>, audit_trail: &mut Vec<MerkleProofHash>) {
        let leaf_node: Arc<Mutex<Box<Node>>> = Arc::new(Mutex::new(Box::new(Node::new())));
        let act_hash = Vec::from(leaf_hash);
        let len = (self.base.lock().unwrap().first().unwrap().len());
        let tem_hash = act_hash.clone();
        for i in 0..len {
            if let Some(first) = (self.base.lock().unwrap().first()) {
                if (first[i].get_hash()) == tem_hash {
                    (*leaf_node.lock().unwrap()).clone_from(&first[i]) ;
                }
            }
        }

        if let Some(parent) = (leaf_node.clone().lock().as_mut()).expect("REASON").get_parent().as_mut() {
            self.build_audit_trail(audit_trail, parent, &mut (&*leaf_node.lock().unwrap()));
        }
    }

}

#[test]
fn test_tree(){

    let mut tree = Tree::new();
    let base_leafs = vec![
        hex_str_to_bytes("leaf1").unwrap(),
        hex_str_to_bytes("leaf2").unwrap(),
        hex_str_to_bytes("leaf3").unwrap(),
        hex_str_to_bytes("leaf4").unwrap(),
    ];
    tree.build_base_leafs(base_leafs);
    tree.build_tree();
   

    let hash = Tree::hash_merkle_branches(
        hex_str_to_bytes("5feceb66ffc86f38d952786c6d696c79c2dbc239dd4e91b46729d73a27fb57e9").unwrap(),
        hex_str_to_bytes("6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b").unwrap(),
    );
    let mut audit_trail: Vec<MerkleProofHash> = Vec::new();
    self.audit_proof(
        hex_str_to_bytes("4a44dc15364204a80fe80e9039455cc1608281820fe2b24f1e5233ade6af1dd5").unwrap(),
        &mut audit_trail,
    );
    self.verify_audit(
        hex_str_to_bytes("a901f842b0016f1e350d20b751851a7179e26dfbb74b213c7a92d37f3c4fbb6c").unwrap(),
        hex_str_to_bytes("4a44dc15364204a80fe80e9039455cc1608281820fe2b24f1e5233ade6af1dd5").unwrap(),
        &audit_trail,
    );

}


