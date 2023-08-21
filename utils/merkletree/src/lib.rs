use std::{collections::LinkedList, sync::{Mutex, Arc}};

#[derive(Clone)]
struct Node {
    parent: Option<Box<Node>>,
    children: [Option<Box<Node>>; 2],
    hash_str: String,
}

impl  PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.parent == other.parent && self.children == other.children && self.hash_str == other.hash_str
    }
}

impl Node {
    pub fn new() -> Node {
        Node {
            parent: None,
            children: [None, None],
            hash_str: String::new(),
        }
    }

    pub fn set_hash(&mut self, hash_str: String) {
        self.hash_str = hash_str;
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

    pub fn get_hash(&self) -> &str {
        &self.hash_str
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

struct Tree {
    base: Arc<Mutex<Vec<Vec<Box<Node>>>>>,
    merkle_root: String,
}

impl Tree {
    fn new() -> Tree {
        Tree {
            base: Arc::new(Mutex::new(Vec::new())),
            merkle_root: String::new(),
        }
    }

    fn make_binary(&mut self) -> usize {
        let vect_size = (self.base.lock().unwrap().last_mut().unwrap()).len();
        if vect_size % 2 != 0 {
            (self.base.lock().unwrap().last_mut().unwrap()).push(self.base.lock().unwrap().last_mut().unwrap()[vect_size - 1].clone());
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
                new_parent.set_parent(self.base.lock().unwrap().last_mut().unwrap()[i].clone());
                new_parent.set_parent(self.base.lock().unwrap().last_mut().unwrap()[i + 1].clone());

                let hash_str = format!(
                    "{}{}",
                    self.base.lock().unwrap().last().unwrap()[i].get_hash(),
                    self.base.lock().unwrap().last().unwrap()[i + 1].get_hash()
                );
                new_parent.set_hash(hash_str);

                new_parent.set_children(
                    self.base.lock().unwrap().last().unwrap()[i].clone(),
                    self.base.lock().unwrap().last().unwrap()[i + 1].clone(),
                );

                new_nodes.push(new_parent);

            }

            //self.print_tree_level(&new_nodes);
            (self.base.lock().unwrap().push(new_nodes));


            if (self.base.lock().unwrap().last_mut().unwrap().len()) <= 1 {
                break;
            }
        }

        self.merkle_root = self.base.lock().unwrap().last_mut().unwrap()[0].get_hash().to_string();
    }

    fn print_tree_level(&self, v: &Vec<Box<Node>>) {
        println!();
        for el in v {
            println!("{}", el.get_hash());
        }
    }

    fn build_base_leafs(&mut self, base_leafs: Vec<String>) {
        let mut new_nodes: Vec<Box<Node>> = Vec::new();
        for leaf in base_leafs {
            let mut new_node = Box::new(Node::new());
            new_node.set_hash(leaf.clone());
            new_nodes.push(new_node);
        }

        self.base.lock().unwrap().push(new_nodes);
    }

    fn verify(&self, hash: &str) -> bool {
        let mut el_node: Option<Box<Node>> = None;
        let act_hash = hash.to_string();

        for i in 0..(self.base.lock().unwrap().first().unwrap().len()) {
            if (self.base.lock().unwrap().first().unwrap()[i].get_hash()) == hash {
                el_node = Some(self.base.lock().unwrap().first().unwrap()[i].clone());
            }
        }

        if el_node.is_none() {
            return false;
        }

        println!("Hash verify: {}", act_hash);

        let mut el_node = el_node.unwrap();
        loop {
            match el_node.get_parent() {
                Some(p) => el_node = p.clone(),
                None => break,
            }
        }

        if act_hash == self.merkle_root {
            return true;
        } else {
            return false;
        }
    }


    fn hash_merkle_branches(left: &str, right: &str) -> String {
        format!("{}{}", left, right)
    }

    fn build_audit_trail(
        &self,
        audit_trail: &mut Vec<MerkleProofHash>,
        parent: &Box<Node>,
        child: &Box<Node>,
    ) {
        if let Some(next_child) = parent.get_children(0) {
            if next_child.eq(child) {
                audit_trail.push(MerkleProofHash {
                    hash: parent.get_children(1).unwrap().get_hash().to_string(),
                    direction: MerkleProofHashBranch::Left,
                });
            } else {
                audit_trail.push(MerkleProofHash {
                    hash: parent.get_children(0).unwrap().get_hash().to_string(),
                    direction: MerkleProofHashBranch::Right,
                });
            }
        }

        if let Some(p) = parent.get_parent() {
            self.build_audit_trail(audit_trail, p, parent);
        }
    }

    fn verify_audit(
        &self,
        root_hash: &str,
        leaf_hash: &str,
        audit_trail: &Vec<MerkleProofHash>,
    ) -> bool {
        if audit_trail.is_empty() {
            return false;
        }

        let mut test_hash = leaf_hash.to_string();

        for proof_hash in audit_trail {
            if proof_hash.direction == MerkleProofHashBranch::Left {
                test_hash = Self::hash_merkle_branches(&test_hash, &proof_hash.hash);
            } else {
                test_hash = Self::hash_merkle_branches(&proof_hash.hash, &test_hash);
            }
        }

        return root_hash == test_hash;
    }

    fn audit_proof(&self, leaf_hash: &str, audit_trail: &mut Vec<MerkleProofHash>) {
        let mut leaf_node: Option<Box<Node>> = None;
        let act_hash = leaf_hash.to_string();

        for i in 0..(self.base.lock().unwrap().first().unwrap().len()) {
            if (self.base.lock().unwrap().first().unwrap()[i].get_hash()) == leaf_hash {
                leaf_node = Some(self.base.lock().unwrap().first().unwrap()[i].clone());
            }
        }

        if let Some(leaf_node) = leaf_node {
            if let Some(parent) = leaf_node.get_parent() {
                self.build_audit_trail(audit_trail, parent, &leaf_node);
            }
        }
    }

    // fn test_verify_audit(&self) {
    //     let hash = Self::hash_merkle_branches(
    //         "5feceb66ffc86f38d952786c6d696c79c2dbc239dd4e91b46729d73a27fb57e9",
    //         "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
    //     );
    //     let mut audit_trail: Vec<MerkleProofHash> = Vec::new();
    //     self.audit_proof(
    //         "4a44dc15364204a80fe80e9039455cc1608281820fe2b24f1e5233ade6af1dd5",
    //         &mut audit_trail,
    //     );
    //     self.verify_audit(
    //         "a901f842b0016f1e350d20b751851a7179e26dfbb74b213c7a92d37f3c4fbb6c",
    //         "4a44dc15364204a80fe80e9039455cc1608281820fe2b24f1e5233ade6af1dd5",
    //         &audit_trail,
    //     );
    // }
}

struct MerkleProofHash {
    hash: String,
    direction: MerkleProofHashBranch,
}

#[derive(PartialEq)]
enum MerkleProofHashBranch {
    Left,
    Right,
}

// fn main() {
//     let mut tree = Tree::new();
//     let base_leafs = vec![
//         "leaf1".to_string(),
//         "leaf2".to_string(),
//         "leaf3".to_string(),
//         "leaf4".to_string(),
//     ];
//     tree.build_base_leafs(base_leafs);
//     tree.build_tree();
//     tree.test_verify_audit();
// }