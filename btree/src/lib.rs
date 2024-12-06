use std::cell::RefCell;
use std::rc::Rc;

pub mod storage;

#[cfg(test)]
mod test;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("I/O error: {0}")]
  IO(#[from] std::io::Error),

  #[error("Serialization failed: {0}")]
  Serialize(#[from] bincode::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct BTree<KEY, VALUE, const S: usize>
where
  KEY: Ord + Clone,
  VALUE: Copy,
{
  root: Rc<RefCell<Node<KEY, VALUE, S>>>,
}

impl<KEY, VALUE, const S: usize> BTree<KEY, VALUE, S>
where
  KEY: Ord + Clone,
  VALUE: Copy,
{
  pub fn new() -> Self {
    BTree {
      root: Rc::new(RefCell::new(Node::<KEY, VALUE, S>::new(true))),
    }
  }

  /// この B-Tree に格納されているキーの下図を参照します。
  ///
  pub fn size(&self) -> usize {
    self.root.borrow().size()
  }

  // この B-Tree の葉までの深さを参照します。この機能は葉を 1 と数えます。
  //
  pub fn level(&self) -> usize {
    self.root.borrow().level(0)
  }

  /// 指定されたキーに関連付けられた値を返します。値が存在しない場合は None を返します。
  ///
  pub fn get(&self, key: &KEY) -> Option<VALUE> {
    self.root.borrow().lookup(key)
  }

  /// ツリーに Key-Value ペアを挿入します。既に同じキーが存在する場合は新しい値で置き換えて古い値を返します。
  ///
  pub fn put(&mut self, key: KEY, value: VALUE) -> Option<VALUE> {
    let (prop, result) = self.root.borrow_mut().upsert(key, value);
    if let Some((keyval, pivot)) = prop {
      let mut new_root = Node::new(false);
      new_root.keys.push(keyval);
      new_root.pivots.push(self.root.clone());
      new_root.pivots.push(Rc::new(RefCell::new(pivot)));
      self.root = Rc::new(RefCell::new(new_root));
    }
    result
  }

  pub fn delete(&mut self, key: &KEY) -> Option<VALUE> {
    let old_value = self.root.borrow_mut().delete(key);
    if !self.root.borrow().is_leaf && self.root.borrow().pivots.len() == 1 {
      let new_root = self.root.borrow().pivots[0].clone();
      self.root = new_root;
    }
    old_value
  }
}

impl<KEY, VALUE, const S: usize> Default for BTree<KEY, VALUE, S>
where
  KEY: Ord + Clone,
  VALUE: Copy,
{
  fn default() -> Self {
    BTree::new()
  }
}

#[derive(Debug)]
struct Node<KEY, VALUE, const S: usize>
where
  KEY: Ord + Clone,
  VALUE: Copy,
{
  is_leaf: bool,
  keys: Vec<KeyVal<KEY, VALUE>>,
  pivots: Vec<Rc<RefCell<Node<KEY, VALUE, S>>>>,
}

impl<KEY, VALUE, const S: usize> Node<KEY, VALUE, S>
where
  KEY: Ord + Clone,
  VALUE: Copy,
{
  fn new(is_leaf: bool) -> Self {
    Node {
      is_leaf,
      keys: Vec::with_capacity(S),
      pivots: Vec::with_capacity(S + 1),
    }
  }

  /// 指定されたキーのインデックスを返します。このノードに一致するキーが存在する場合は `Ok` と共にその
  /// インデックスを返します。存在しない場合は `Err` と共に `key` が存在すべきインデックスを返します。
  ///
  #[inline]
  fn find_index(&self, key: &KEY) -> std::result::Result<usize, usize> {
    self.keys.binary_search_by(|prove| prove.key.cmp(key))
  }

  fn size(&self) -> usize {
    let mut size = self.keys.len();
    if !self.is_leaf {
      size += self
        .pivots
        .iter()
        .map(|child| child.borrow().size())
        .sum::<usize>();
    }
    size
  }

  fn level(&self, level: usize) -> usize {
    if self.is_leaf {
      level + 1
    } else {
      self.pivots[0].borrow().level(level + 1)
    }
  }

  /// このノードをルートとする部分木から指定されたキーに関連付けられた値を検索します。
  ///
  fn lookup(&self, key: &KEY) -> Option<VALUE> {
    match self.find_index(key) {
      Ok(i) => Some(self.keys[i].value),
      Err(i) => {
        if self.is_leaf {
          None
        } else {
          self.pivots[i].borrow().lookup(key)
        }
      }
    }
  }

  /// このノードをルートとする部分木に指定された Key-Value を追加します。すでに同じキーが存在する場合は
  /// 値を更新する UPSERT の動作となります。
  ///
  fn upsert(&mut self, key: KEY, value: VALUE) -> (SplitPropagation<KEY, VALUE, S>, Option<VALUE>) {
    match self.find_index(&key) {
      Ok(i) => {
        // 既にキーが存在する場合はその値を置き換えて以前の値を返す
        let old_value = self.keys[i].value;
        self.keys[i].value = value;
        (None, Some(old_value))
      }
      Err(i) => {
        if self.is_leaf {
          self.keys.insert(i, KeyVal::new(key, value));
          let parent_insertion = self.split();
          (parent_insertion, None)
        } else {
          let (new_node, old_value) = self.pivots[i].borrow_mut().upsert(key, value);
          if let Some((keyval, node)) = new_node {
            self.keys.insert(i, keyval);
            self.pivots.insert(i + 1, Rc::new(RefCell::new(node)));
            let parent_insertion = self.split();
            (parent_insertion, old_value)
          } else {
            (None, old_value)
          }
        }
      }
    }
  }

  /// このノードのキー数が `2S` を超えていれば分割を行います。
  ///
  fn split(&mut self) -> SplitPropagation<KEY, VALUE, S> {
    debug_assert!(self.is_leaf || self.keys.len() + 1 == self.pivots.len());
    if self.keys.len() == 2 * S + 1 {
      let mut right_node = Node::new(self.is_leaf);
      right_node.keys = self.keys.split_off(S);
      let keyval = right_node.keys.remove(0);
      if !self.is_leaf {
        right_node.pivots = self.pivots.split_off(S + 1);
      }
      debug_assert_eq!(S, self.keys.len());
      debug_assert_eq!(S, right_node.keys.len());
      Some((keyval, right_node))
    } else {
      debug_assert!(self.keys.len() <= 2 * S);
      None
    }
  }

  fn delete(&mut self, key: &KEY) -> Option<VALUE> {
    match self.find_index(key) {
      Ok(i) if self.is_leaf => Some(self.keys.remove(i).value),
      Err(_) if self.is_leaf => None,
      Ok(i) => {
        let old_value = self.keys[i].value;
        let mut left = self.pivots[i].borrow_mut();
        let mut right = self.pivots[i + 1].borrow_mut();
        if let Some(keyval) = left
          .remove_most_leftright(false, false)
          .or_else(|| right.remove_most_leftright(true, false))
        {
          self.keys[i] = keyval;
        } else {
          let remove_from_left = i % 2 == 0;
          let child = if remove_from_left {
            &mut left
          } else {
            &mut right
          };
          let keyval = child
            .remove_most_leftright(!remove_from_left, true)
            .unwrap();
          self.keys[i] = keyval;
          child.rebalance_most_leftright(!remove_from_left);
          drop(left);
          drop(right);
          self.rebalance(i + if remove_from_left { 0 } else { 1 });
        }
        Some(old_value)
      }
      Err(i) => {
        let old_value = self.pivots[i].borrow_mut().delete(key);
        self.rebalance(i);
        old_value
      }
    }
  }

  fn remove_most_leftright(&mut self, leftmost: bool, force: bool) -> Option<KeyVal<KEY, VALUE>> {
    if !self.is_leaf {
      let i = if leftmost { 0 } else { self.pivots.len() - 1 };
      self.pivots[i]
        .borrow_mut()
        .remove_most_leftright(leftmost, force)
    } else if self.keys.len() > S || force {
      let keyval = if leftmost {
        self.keys.remove(0)
      } else {
        self.keys.pop().unwrap()
      };
      Some(keyval)
    } else {
      None
    }
  }

  fn rebalance_most_leftright(&mut self, leftmost: bool) {
    if !self.is_leaf {
      let i = if leftmost { 0 } else { self.pivots.len() - 1 };
      self.pivots[i]
        .borrow_mut()
        .rebalance_most_leftright(leftmost);
      self.rebalance(i);
    }
  }

  fn rebalance(&mut self, i: usize) {
    if self.pivots[i].borrow().keys.len() >= S {
      return;
    }
    if i + 1 < self.pivots.len() && self.pivots[i + 1].borrow().keys.len() > S {
      // 右ノードのキーを再配分
      let mut left = self.pivots[i].borrow_mut();
      let mut right = self.pivots[i + 1].borrow_mut();
      left.keys.push(self.keys[i].clone());
      self.keys[i] = right.keys.remove(0);
      assert!(left.is_leaf == right.is_leaf);
      if !left.is_leaf {
        left.pivots.push(right.pivots.remove(0));
      }
    } else if i != 0 && self.pivots[i - 1].borrow().keys.len() > S {
      // 左ノードのキーを再配分
      let mut right = self.pivots[i].borrow_mut();
      let mut left = self.pivots[i - 1].borrow_mut();
      right.keys.insert(0, self.keys[i - 1].clone());
      self.keys[i - 1] = left.keys.pop().unwrap();
      assert!(right.is_leaf == left.is_leaf);
      if !right.is_leaf {
        right.pivots.insert(0, left.pivots.pop().unwrap());
      }
    } else if i + 1 < self.pivots.len() {
      // 右ノードとマージ
      let kv = self.keys.remove(i);
      let right_rc = self.pivots.remove(i + 1);
      let mut left = self.pivots[i].borrow_mut();
      let mut right = right_rc.borrow_mut();
      left.keys.push(kv);
      left.keys.append(&mut right.keys);
      assert!(left.is_leaf == right.is_leaf);
      if !left.is_leaf {
        left.pivots.append(&mut right.pivots);
      }
    } else {
      // 左ノードとマージ
      let kv = self.keys.remove(i - 1);
      let right_rc = self.pivots.remove(i);
      let mut right = right_rc.borrow_mut();
      let mut left = self.pivots[i - 1].borrow_mut();
      left.keys.push(kv);
      left.keys.append(&mut right.keys);
      assert!(left.is_leaf == right.is_leaf);
      if !left.is_leaf {
        left.pivots.append(&mut right.pivots);
      }
    }
  }
}

#[derive(Debug, Clone)]
struct KeyVal<KEY, VALUE>
where
  KEY: Clone,
  VALUE: Clone,
{
  key: KEY,
  value: VALUE,
}

impl<KEY, VALUE> KeyVal<KEY, VALUE>
where
  KEY: Clone,
  VALUE: Clone,
{
  fn new(key: KEY, value: VALUE) -> Self {
    KeyVal { key, value }
  }
}

type SplitPropagation<KEY, VALUE, const S: usize> =
  Option<(KeyVal<KEY, VALUE>, Node<KEY, VALUE, S>)>;
