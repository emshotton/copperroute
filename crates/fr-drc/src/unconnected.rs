use fr_board::ItemId;

#[derive(Debug, Clone, PartialEq)]
pub struct UnconnectedItems {
            pub first_item: ItemId,
                pub second_item: Option<ItemId>,
            pub all_items: Vec<ItemId>,
        pub kind: UnconnectedKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnconnectedKind {
        UnconnectedItems,
        TrackDangling,
        ViaDangling,
}

impl UnconnectedItems {
            pub fn new_pair(first_item: ItemId, second_item: ItemId) -> Self {
        UnconnectedItems {
            first_item,
            second_item: Some(second_item),
            all_items: vec![first_item, second_item],
            kind: UnconnectedKind::UnconnectedItems,
        }
    }

            pub fn new_with_all_items(
        first_item: ItemId,
        second_item: ItemId,
        all_items: Vec<ItemId>,
    ) -> Self {
        UnconnectedItems {
            first_item,
            second_item: Some(second_item),
            all_items,
            kind: UnconnectedKind::UnconnectedItems,
        }
    }

                                                                    pub fn new_typed(
        first_item: ItemId,
        second_item: Option<ItemId>,
        kind: UnconnectedKind,
    ) -> Self {
        UnconnectedItems {
            first_item,
            second_item,
            all_items: std::iter::once(first_item).chain(second_item).collect(),
            kind,
        }
    }
}
