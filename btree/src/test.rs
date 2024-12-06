use rand::{RngCore, SeedableRng};

use crate::{BTree, Node};
use std::cell::Ref;
use std::collections::HashMap;
use std::fmt::Debug;
use std::{cell::RefCell, rc::Rc};

#[test]
fn basic_structure_change() {
  let mut btree = BTree::<_, _, 2>::new();
  for i in 0..=3 {
    btree.put(i, i);
  }
  assert!(btree.root.borrow().is_leaf);
  assert_eq!(1, btree.level());
  assert_eq!(4, btree.root.borrow().keys.len());

  // split
  btree.put(4, 4);
  assert!(!btree.root.borrow().is_leaf);
  assert_eq!(2, btree.level());
  assert_eq!(1, btree.root.borrow().keys.len());
  assert_eq!(2, btree.root.borrow().keys[0].key);
  assert_eq!(2, btree.root.borrow().pivots[0].borrow().keys.len());
  assert_eq!(2, btree.root.borrow().pivots[1].borrow().keys.len());

  // split
  btree.put(5, 5);
  btree.put(6, 6);
  btree.put(7, 7);
  assert!(!btree.root.borrow().is_leaf);
  assert_eq!(2, btree.level());
  assert_eq!(2, btree.root.borrow().keys.len());
  assert_eq!(2, btree.root.borrow().keys[0].key);
  assert_eq!(5, btree.root.borrow().keys[1].key);
  assert_eq!(2, btree.root.borrow().pivots[0].borrow().keys.len());
  assert_eq!(2, btree.root.borrow().pivots[1].borrow().keys.len());
  assert_eq!(2, btree.root.borrow().pivots[2].borrow().keys.len());

  // split × 2
  for i in 7..=13 {
    btree.put(i, i);
  }
  assert!(!btree.root.borrow().is_leaf);
  assert_eq!(2, btree.level());
  assert_eq!(4, btree.root.borrow().keys.len());

  // split with level up
  for i in 14..=16 {
    btree.put(i, i);
  }
  assert!(!btree.root.borrow().is_leaf);
  assert_eq!(3, btree.level());
  assert_eq!(1, btree.root.borrow().keys.len());

  // get
  for i in 0..=16 {
    assert_eq!(Some(i), btree.get(&i));
  }

  // delete with merge leftmost key in leaf
  assert_eq!(Some(0), btree.delete(&0));
  assert_eq!(2, btree.level());
  assert_eq!(4, btree.root.borrow().pivots[0].borrow().keys.len());
  assert_eq!(1, btree.root.borrow().pivots[0].borrow().keys[0].key);
  assert_eq!(2, btree.root.borrow().pivots[0].borrow().keys[1].key);
  assert_eq!(3, btree.root.borrow().pivots[0].borrow().keys[2].key);
  assert_eq!(4, btree.root.borrow().pivots[0].borrow().keys[3].key);
  assert_eq!(5, btree.root.borrow().keys[0].key);
  btree.put(0, 0);

  // delete with merge rightmost key in leaf
  assert_eq!(Some(16), btree.delete(&16));
  assert_eq!(2, btree.level());
  assert_eq!(4, btree.root.borrow().pivots[4].borrow().keys.len());
  assert_eq!(12, btree.root.borrow().pivots[4].borrow().keys[0].key);
  assert_eq!(13, btree.root.borrow().pivots[4].borrow().keys[1].key);
  assert_eq!(14, btree.root.borrow().pivots[4].borrow().keys[2].key);
  assert_eq!(15, btree.root.borrow().pivots[4].borrow().keys[3].key);
  assert_eq!(11, btree.root.borrow().keys[3].key);
  btree.put(16, 16);

  // delete
  dump(0, btree.root.clone());
  println!("-----");
  assert_eq!(Some(8), btree.delete(&8));
  dump(0, btree.root.clone());
  assert_eq!(2, btree.level());
  assert_eq!(4, btree.root.borrow().pivots[1].borrow().keys.len());
  assert_eq!(3, btree.root.borrow().pivots[1].borrow().keys[0].key);
  assert_eq!(4, btree.root.borrow().pivots[1].borrow().keys[1].key);
  assert_eq!(5, btree.root.borrow().pivots[1].borrow().keys[2].key);
  assert_eq!(6, btree.root.borrow().pivots[1].borrow().keys[3].key);
  assert_eq!(7, btree.root.borrow().keys[1].key);
  btree.put(16, 16);
}

#[test]
fn sequential_put_delete() {
  const MAX: usize = 1000;
  let mut btree = BTree::<_, _, 3>::new();
  for i in 0usize..MAX {
    btree.put(i, i);
    validate(&btree);
  }

  for i in 0usize..MAX {
    assert_eq!(Some(i), btree.get(&i));
  }

  assert_eq!(MAX, btree.size());
  assert_eq!(None, btree.delete(&MAX));
  assert_eq!(None, btree.delete(&(MAX + 1)));
  assert_eq!(MAX, btree.size());
  for i in 0usize..MAX {
    let key = MAX - i - 1;
    assert_eq!(MAX - i, btree.size());
    assert_eq!(Some(key), btree.delete(&key));
    assert_eq!(MAX - i - 1, btree.size());
    validate(&btree);
  }
  assert_eq!(0, btree.size());
}

#[test]
fn random_put_delete() {
  const MAX: usize = 1000;
  let mut btree = BTree::<_, _, 3>::new();
  let seed = 4u64;
  let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
  let mut expecteds = HashMap::with_capacity(MAX);
  for _ in 0usize..MAX {
    let (key, value) = loop {
      let key = rng.next_u32();
      if !expecteds.contains_key(&key) {
        break (key, rng.next_u64());
      }
    };
    expecteds.insert(key, value);
    btree.put(key, value);
    validate(&btree);
  }

  for (key, expected) in expecteds.iter().take(MAX) {
    assert_eq!(Some(*expected), btree.get(key));
  }

  assert_eq!(MAX, btree.size());
  let unregistered = (0u32..).find(|i| !expecteds.contains_key(i)).unwrap();
  assert_eq!(None, btree.delete(&unregistered));
  assert_eq!(MAX, btree.size());
  for (i, (key, expected)) in expecteds.iter().enumerate() {
    println!("----delete {i:?} [{key:?}] {expected:?}");
    if let Some(value) = btree.delete(key) {
      assert_eq!(*expected, value);
      assert_eq!(MAX - i - 1, btree.size());
      validate(&btree);
      dump(0, btree.root.clone());
    } else {
      validate(&btree);
      dump(0, btree.root.clone());
      panic!();
    }
  }
  assert_eq!(0, btree.size());
}

#[test]
fn fixed_random_put_delele() {
  let mut btree = BTree::<_, _, 2>::new();
  let expecteds = [
    3281021079u32,
    6451452,
    2978716138,
    1490858745,
    3771312625,
    1946430169,
    3119100097,
    11229054,
    2340205904,
    832773000,
  ];

  validate(&btree);
  println!("{expecteds:?}");
  for key in expecteds.iter() {
    btree.put(*key, *key);
    validate(&btree);
  }

  for key in expecteds.iter() {
    dump(0, btree.root.clone());
    if let Some(value) = btree.delete(key) {
      assert_eq!(*key, value);
      validate(&btree);
    } else {
      dump(0, btree.root.clone());
      panic!();
    }
  }
}

fn dump<KEY, VALUE, const S: usize>(indent: usize, node: Rc<RefCell<Node<KEY, VALUE, S>>>)
where
  KEY: Ord + Clone + Debug,
  VALUE: Copy,
{
  if node.borrow().is_leaf {
    println!(
      "{}{:?}",
      " ".repeat(indent),
      node
        .borrow()
        .keys
        .iter()
        .map(|kv| kv.key.clone())
        .collect::<Vec<_>>()
    );
  } else {
    for i in 0..node.borrow().keys.len() {
      dump(indent + 2, node.borrow().pivots[i].clone());
      println!("{}{:?}", " ".repeat(indent), node.borrow().keys[i].key);
    }
    dump(
      indent + 2,
      node.borrow().pivots[node.borrow().pivots.len() - 1].clone(),
    );
  }
}

fn validate<KEY, VALUE, const S: usize>(btree: &BTree<KEY, VALUE, S>)
where
  KEY: Ord + Clone + Debug,
  VALUE: Copy,
{
  if let Err(msg) = _validate(btree.root.borrow(), true, 0) {
    println!("{}", msg);
    dump(0, btree.root.clone());
    panic!("validation failed: {msg}");
  }
}

fn _validate<KEY, VALUE, const S: usize>(
  node: Ref<Node<KEY, VALUE, S>>,
  root: bool,
  _depth: usize,
) -> std::result::Result<usize, String>
where
  KEY: Ord + Clone + Debug,
  VALUE: Copy,
{
  let (min, max) = match (root, node.is_leaf) {
    (true, true) => (0, 2 * S),
    (true, false) => (1, 2 * S),
    (false, true) => (S, 2 * S),
    (false, false) => (S, 2 * S),
  };
  if node.keys.len() < min || node.keys.len() > max {
    return Err(format!(
      "[{}] The number of keys is not in the range {:?} to {:?}: {:?}",
      _depth,
      min,
      max,
      node.keys.len()
    ));
  }
  if !node.is_leaf {
    if node.pivots.len() < min + 1 || node.keys.len() > max + 1 {
      return Err(format!(
        "[{}] The number of pivots is not in the range {:?} to {:?}: {:?}",
        _depth,
        min + 1,
        max + 1,
        node.pivots.len()
      ));
    }
    let mut depths = Vec::with_capacity(node.pivots.len());
    for child in node.pivots.iter() {
      let depth = _validate(child.borrow(), false, _depth + 1)?;
      depths.push(depth);
    }
    let depth = depths.first().unwrap();
    if !depths.iter().all(|d| d == depth) {
      return Err(format!(
        "[{}] The tree level is not match: {:?}",
        _depth, depths
      ));
    }
    return Ok(*depth);
  } else if !node.pivots.is_empty() {
    return Err(format!(
      "[{}] Pivots are not empty: {:?}",
      _depth,
      node.pivots.len()
    ));
  }

  // all keys are sorted
  if !node.keys.is_empty() {
    for i in 0..node.keys.len() - 1 {
      if node.keys[i].key >= node.keys[i + 1].key {
        return Err(format!(
          "[{}] Keys are not sorted: key[{}] ≧ key[{}",
          _depth,
          i,
          i + 1
        ));
      }
    }
  }

  // all branches are smaller or larger
  if !node.is_leaf {
    for i in 0..node.keys.len() {
      if node.pivots[i].borrow().keys.last().unwrap().key >= node.keys[i].key {
        return Err(format!(
          "[{}] The left branch of key[{}] is out of order",
          _depth, i
        ));
      }
      if node.pivots[i + 1].borrow().keys.first().unwrap().key <= node.keys[i].key {
        return Err(format!(
          "[{}] The left branch of key[{}] is out of order",
          _depth, i
        ));
      }
    }
  }
  Ok(_depth)
}
