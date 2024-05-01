use crypto_hash::{Algorithm, hex_digest};
use std::cmp;

#[derive(Debug, Clone)]
struct MerkleNode {
    hash: String,
    left: Option<Box<MerkleNode>>,
    right: Option<Box<MerkleNode>>,
}

impl MerkleNode {
    fn new(hash: String, left: Option<Box<MerkleNode>>, right: Option<Box<MerkleNode>>) -> MerkleNode {
        MerkleNode { hash, left, right }
    }

    fn compute_hash(data: &str) -> String {
        hex_digest(Algorithm::SHA256, data.as_bytes()) // returns SHA256 String
    }
}

#[derive(Debug)]
struct MerkleTree {
    root: Option<Box<MerkleNode>>,
}

impl MerkleTree {
    fn new(data: Vec<&str>) -> MerkleTree {
        let nodes = data.iter().map(|d| MerkleNode::new(MerkleNode::compute_hash(d), None, None)).collect::<Vec<_>>();
        MerkleTree { root: Some(Box::new(MerkleTree::build_tree(nodes))) }
    }

    fn build_tree(mut nodes: Vec<MerkleNode>) -> MerkleNode {
        if nodes.len() == 1 {
            return nodes.remove(0);
        }

        let mut parents = Vec::new();
        for i in (0..nodes.len()).step_by(2) {
            let left = nodes[i].clone();
            let right = if i + 1 < nodes.len() {
                nodes[i + 1].clone()
            } else {
                left.clone()
            };

            let hash = MerkleNode::compute_hash(&(left.hash.clone() + &right.hash));
            let parent = MerkleNode::new(hash, Some(Box::new(left)), Some(Box::new(right)));
            parents.push(parent);
        }

        MerkleTree::build_tree(parents)
    }

    fn root_hash(&self) -> Option<String> {
        match &self.root {
            Some(node) => Some(node.hash.clone()),
            None => None,
        }
    }

    fn print_tree(&self) {
        self.print_node(&self.root, 0);
    }

    fn print_node(&self, node: &Option<Box<MerkleNode>>, depth: usize) {
        if let Some(n) = node {
            self.print_node(&n.left, depth + 1);
            println!("{:indent$}{}", "", n.hash, indent = depth * 2);
            self.print_node(&n.right, depth + 1);
        }
    }
}

fn main() {
    let data = vec![
        "Transaction 1",
        "Transaction 2",
        "Transaction 3",
        "Transaction 4",
        "Transaction 5",
    ];
    let merkle_tree = MerkleTree::new(data.clone());

    println!("Merkle Tree:");
    merkle_tree.print_tree();

    if let Some(root_hash) = merkle_tree.root_hash() {
        println!("Root hash: {}", root_hash);
    } else {
        println!("Merkle Tree is empty.");
    }
}
