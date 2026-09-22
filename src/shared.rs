use crate::{Attribute, Dom, Kind, Node, NodeId, Text};

pub trait DomSource {
    fn node_count(&self) -> usize;
    fn kind(&self, node: NodeId) -> Kind;
    fn tag(&self, node: NodeId) -> &str;
    fn data(&self, node: NodeId) -> &str;
    fn parent(&self, node: NodeId) -> Option<NodeId>;
    fn children(&self, node: NodeId) -> &[NodeId];
    fn attribute_count(&self, node: NodeId) -> usize;
    fn attribute(&self, node: NodeId, attribute: usize) -> (&str, &str, &str);

    fn original_tag(&self, node: NodeId) -> &str {
        self.tag(node)
    }

    fn namespace(&self, _node: NodeId) -> &str {
        ""
    }

    fn copy_for_readability(&self) -> Dom {
        let mut nodes = Vec::with_capacity(self.node_count());
        for index in 0..self.node_count() {
            let mut node = Node::new(self.kind(index));
            node.tag = self.tag(index).into();
            node.data = self.data(index).into();
            node.parent = self.parent(index);
            node.children.extend_from_slice(self.children(index));
            node.attrs = (0..self.attribute_count(index))
                .map(|attribute| {
                    let (namespace, key, value) = self.attribute(index, attribute);
                    Attribute {
                        namespace: namespace.into(),
                        key: key.into(),
                        value: value.into(),
                    }
                })
                .collect();
            nodes.push(node);
        }
        Dom {
            document: crate::Document { nodes },
            atoms: (0..self.node_count())
                .map(|node| crate::dom::original_atom(self.original_tag(node)))
                .collect(),
            namespaces: (0..self.node_count())
                .map(|node| Text::from(self.namespace(node)))
                .collect(),
        }
    }
}

impl DomSource for Dom {
    fn node_count(&self) -> usize {
        self.nodes.len()
    }

    fn kind(&self, node: NodeId) -> Kind {
        self.nodes[node].kind
    }

    fn tag(&self, node: NodeId) -> &str {
        &self.nodes[node].tag
    }

    fn data(&self, node: NodeId) -> &str {
        &self.nodes[node].data
    }

    fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.nodes[node].parent
    }

    fn children(&self, node: NodeId) -> &[NodeId] {
        &self.nodes[node].children
    }

    fn attribute_count(&self, node: NodeId) -> usize {
        self.nodes[node].attrs.len()
    }

    fn attribute(&self, node: NodeId, attribute: usize) -> (&str, &str, &str) {
        let attribute = &self.nodes[node].attrs[attribute];
        (&attribute.namespace, &attribute.key, &attribute.value)
    }

    fn original_tag(&self, node: NodeId) -> &str {
        &self.atoms[node]
    }

    fn namespace(&self, node: NodeId) -> &str {
        &self.namespaces[node]
    }

    fn copy_for_readability(&self) -> Dom {
        self.clone()
    }
}
