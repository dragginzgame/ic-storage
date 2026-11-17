use super::{Error, Image, Map, Node, arena};
use alloc::vec::Vec;
use core::{alloc::Allocator, cmp::Ordering};

impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> Map<K, V, A, P> {
    pub(super) fn key(&self, id: u64) -> &[u8] {
        self.node(id).key.bytes(&self.image.payload)
    }
    pub(super) fn value(&self, id: u64) -> &[u8] {
        self.node(id).value.bytes(&self.image.payload)
    }
    fn remove_min(&mut self, root: u64) -> (u64, arena::Span, arena::Span) {
        let node = *self.node(root);
        if node.left == 0 {
            self.retire(root);
            return (node.right, node.key, node.value);
        }
        let (left, key, value) = self.remove_min(node.left);
        self.node_mut(root).left = left;
        (self.balance(root), key, value)
    }
    pub(super) fn node(&self, id: u64) -> &Node<K, V> {
        &self.image.nodes[id as usize - 1]
    }
    pub(super) fn node_mut(&mut self, id: u64) -> &mut Node<K, V> {
        &mut self.image.nodes[id as usize - 1]
    }
    pub(super) fn find(&self, key: &[u8]) -> u64 {
        let mut id = self.image.root;
        while id != 0 {
            let node = self.node(id);
            id = match key.cmp(self.key(id)) {
                Ordering::Less => node.left,
                Ordering::Greater => node.right,
                Ordering::Equal => return id,
            };
        }
        0
    }
    // Node and payload capacity were reserved together before record mutation.
    pub(super) fn allocate(&mut self, node: Node<K, V>) -> u64 {
        if self.image.free != 0 {
            let id = self.image.free;
            self.image.free = self.node(id).left;
            *self.node_mut(id) = node;
            id
        } else {
            self.image.nodes.push(node);
            self.image.nodes.len() as u64
        }
    }
    fn retire(&mut self, id: u64) {
        *self.node_mut(id) = Node {
            left: self.image.free,
            ..Node::EMPTY
        };
        self.image.free = id;
    }
    fn height(&self, id: u64) -> u64 {
        if id == 0 { 0 } else { self.node(id).height }
    }
    fn update(&mut self, id: u64) {
        let node = self.node(id);
        let height = 1 + self.height(node.left).max(self.height(node.right));
        self.node_mut(id).height = height;
    }
    fn rotate_left(&mut self, id: u64) -> u64 {
        let pivot = self.node(id).right;
        self.node_mut(id).right = self.node(pivot).left;
        self.node_mut(pivot).left = id;
        self.update(id);
        self.update(pivot);
        pivot
    }
    fn rotate_right(&mut self, id: u64) -> u64 {
        let pivot = self.node(id).left;
        self.node_mut(id).left = self.node(pivot).right;
        self.node_mut(pivot).right = id;
        self.update(id);
        self.update(pivot);
        pivot
    }
    fn balance(&mut self, id: u64) -> u64 {
        self.update(id);
        let left = self.node(id).left;
        let right = self.node(id).right;
        if self.height(left) > self.height(right) + 1 {
            if self.height(self.node(left).right) > self.height(self.node(left).left) {
                let pivot = self.rotate_left(left);
                self.node_mut(id).left = pivot;
            }
            self.rotate_right(id)
        } else if self.height(right) > self.height(left) + 1 {
            if self.height(self.node(right).left) > self.height(self.node(right).right) {
                let pivot = self.rotate_right(right);
                self.node_mut(id).right = pivot;
            }
            self.rotate_left(id)
        } else {
            id
        }
    }
    pub(super) fn insert_node(&mut self, root: u64, id: u64) -> u64 {
        if root == 0 {
            return id;
        }
        if self.key(id) < self.key(root) {
            let left = self.insert_node(self.node(root).left, id);
            self.node_mut(root).left = left;
        } else {
            let right = self.insert_node(self.node(root).right, id);
            self.node_mut(root).right = right;
        }
        self.balance(root)
    }
    pub(super) fn remove_node(&mut self, root: u64, key: &[u8]) -> u64 {
        match key.cmp(self.key(root)) {
            Ordering::Less => {
                let left = self.remove_node(self.node(root).left, key);
                self.node_mut(root).left = left;
            }
            Ordering::Greater => {
                let right = self.remove_node(self.node(root).right, key);
                self.node_mut(root).right = right;
            }
            Ordering::Equal => {
                let left = self.node(root).left;
                let right = self.node(root).right;
                if left == 0 || right == 0 {
                    let node = *self.node(root);
                    arena::retire(&mut self.image.payload, node.key);
                    arena::retire(&mut self.image.payload, node.value);
                    self.retire(root);
                    return if left == 0 { right } else { left };
                }
                // Transfer the successor's spans without copying payloads or
                // freeing bytes now owned by the replacement node.
                let old = *self.node(root);
                let (right, key, value) = self.remove_min(right);
                arena::retire(&mut self.image.payload, old.key);
                arena::retire(&mut self.image.payload, old.value);
                self.node_mut(root).key = key;
                self.node_mut(root).value = value;
                self.node_mut(root).right = right;
            }
        }
        self.balance(root)
    }
}

pub(super) fn validate<const K: usize, const V: usize, A: Allocator, P: Allocator>(
    image: &Image<K, V, A, P>,
    limit: usize,
) -> Result<(), Error> {
    if image.len > limit || image.nodes.len() > limit {
        return Err(Error::NativeImage);
    }
    arena::validate(image)?;
    let mut seen = Vec::new();
    seen.try_reserve_exact(image.nodes.len())
        .map_err(|_| Error::Allocation)?;
    seen.resize(image.nodes.len(), false);
    let (_, count) = visit(image, &mut seen, image.root, None, None, 0)?;
    if count != image.len {
        return Err(Error::NativeImage);
    }
    let mut id = image.free;
    while id != 0 {
        let index = usize::try_from(id - 1).map_err(|_| Error::NativeImage)?;
        let node = image.nodes.get(index).ok_or(Error::NativeImage)?;
        if seen[index]
            || node.height != 0
            || node.right != 0
            || node.key != arena::Span::EMPTY
            || node.value != arena::Span::EMPTY
        {
            return Err(Error::NativeImage);
        }
        seen[index] = true;
        id = node.left;
    }
    if seen.iter().any(|used| !used) {
        return Err(Error::NativeImage);
    }
    Ok(())
}

fn visit<const K: usize, const V: usize, A: Allocator, P: Allocator>(
    image: &Image<K, V, A, P>,
    seen: &mut [bool],
    id: u64,
    lower: Option<&[u8]>,
    upper: Option<&[u8]>,
    depth: usize,
) -> Result<(u64, usize), Error> {
    if id == 0 {
        return Ok((0, 0));
    }
    // Reject malicious depth before recursion can exhaust the validation stack.
    // No representable valid AVL allocation on supported hosts needs 128 levels.
    if depth >= 128 {
        return Err(Error::NativeImage);
    }
    let index = usize::try_from(id - 1).map_err(|_| Error::NativeImage)?;
    let node = image.nodes.get(index).ok_or(Error::NativeImage)?;
    let key = node.key.bytes(&image.payload);
    if seen[index]
        || lower.is_some_and(|lower| key <= lower)
        || upper.is_some_and(|upper| key >= upper)
    {
        return Err(Error::NativeImage);
    }
    seen[index] = true;
    let (lh, lc) = visit(image, seen, node.left, lower, Some(key), depth + 1)?;
    let (rh, rc) = visit(image, seen, node.right, Some(key), upper, depth + 1)?;
    if lh.abs_diff(rh) > 1 || node.height != 1 + lh.max(rh) {
        return Err(Error::NativeImage);
    }
    Ok((node.height, 1 + lc + rc))
}
