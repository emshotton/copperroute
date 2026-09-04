use crate::structure::LayerStructure;

use super::equals_ignore_case;

pub const CLEARANCE_SAFETY_MARGIN: i32 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClearanceMatrix {
            layer_count: usize,
                class_names: Vec<String>,
        values: Vec<i32>,
        row_max_value: Vec<i32>,
        max_value_on_layer: Vec<i32>,
}

impl ClearanceMatrix {
                            pub fn new<S: AsRef<str>>(
        class_count: usize,
        layer_structure: &LayerStructure,
        names: &[S],
    ) -> ClearanceMatrix {
        let class_count = class_count.max(1);
        let layer_count = layer_structure.count();
        ClearanceMatrix {
            layer_count,
            class_names: (0..class_count)
                .map(|i| names[i].as_ref().to_string())
                .collect(),
            values: vec![0; class_count * class_count * layer_count],
            row_max_value: vec![0; class_count * layer_count],
            max_value_on_layer: vec![0; layer_count],
        }
    }

                        pub fn get_default_instance(
        layer_structure: &LayerStructure,
        default_value: i32,
    ) -> ClearanceMatrix {
        let mut result = ClearanceMatrix::new(2, layer_structure, &["null", "default"]);
        result.set_default_value(default_value);
        result
    }

        pub fn get_class_count(&self) -> usize {
        self.class_names.len()
    }

        pub fn get_layer_count(&self) -> usize {
        self.layer_count
    }

                                    fn index(&self, class_i: usize, class_j: usize, layer: usize) -> usize {
        let class_count = self.get_class_count();
        assert!(
            class_i < class_count,
            "clearance class {class_i} out of range (class count {class_count}): \
             Java throws ArrayIndexOutOfBoundsException here (ClearanceMatrix.java:102)"
        );
        assert!(
            class_j < class_count,
            "clearance class {class_j} out of range (class count {class_count}): \
             Java throws ArrayIndexOutOfBoundsException here (ClearanceMatrix.java:101)"
        );
        assert!(
            layer < self.layer_count,
            "layer {layer} out of range (layer count {}): \
             Java throws ArrayIndexOutOfBoundsException here (ClearanceMatrix.java:115)",
            self.layer_count
        );
        (class_j * class_count + class_i) * self.layer_count + layer
    }

            pub fn get_no(&self, name: &str) -> Option<usize> {
        self.class_names
            .iter()
            .position(|n| equals_ignore_case(n, name))
    }

            pub fn get_name(&self, clearance_class_index: usize) -> Option<&str> {
        self.class_names
            .get(clearance_class_index)
            .map(String::as_str)
    }

            pub fn set_default_value(&mut self, value: i32) {
        for layer in 0..self.layer_count {
            self.set_default_value_on_layer(layer, value);
        }
    }

                    pub fn set_default_value_on_layer(&mut self, layer: usize, value: i32) {
        for i in 1..self.get_class_count() {
            for j in 1..self.get_class_count() {
                self.set_value(i, j, layer, value);
            }
        }
    }

            pub fn set_value_on_all_layers(&mut self, class_i: usize, class_j: usize, value: i32) {
        for layer in 0..self.layer_count {
            self.set_value(class_i, class_j, layer, value);
        }
    }

                                                    pub fn set_value(&mut self, class_i: usize, class_j: usize, layer: usize, value: i32) {
        let mut value = value.max(0);
        if value % 2 != 0 {
            if value == i32::MAX {
                value -= 1;
            } else {
                value += 1;
            }
        }

        let index = self.index(class_i, class_j, layer);
        self.values[index] = value;
        let row_index = class_j * self.layer_count + layer;
        self.row_max_value[row_index] = self.row_max_value[row_index].max(value);
        self.max_value_on_layer[layer] = self.max_value_on_layer[layer].max(value);
    }

                pub fn set_inner_value(&mut self, class_i: usize, class_j: usize, value: i32) {
        for layer in 1..self.layer_count.saturating_sub(1) {
            self.set_value(class_i, class_j, layer, value);
        }
    }

                            pub fn get_value(
        &self,
        class_i: usize,
        class_j: usize,
        layer: usize,
        add_safety_margin: bool,
    ) -> i32 {
        let class_count = self.get_class_count();
        if class_i >= class_count || class_j >= class_count || layer >= self.layer_count {
            return 0;
        }
        let value = self.values[self.index(class_i, class_j, layer)];
        if add_safety_margin {
            value + CLEARANCE_SAFETY_MARGIN
        } else {
            value
        }
    }

                                pub fn max_value(&self, class_i: usize, layer: usize) -> i32 {
        let i = class_i.min(self.get_class_count() - 1);
        let layer_index = layer.min(self.layer_count - 1);
        self.row_max_value[i * self.layer_count + layer_index]
    }

            pub fn max_value_on_layer(&self, layer: usize) -> i32 {
        self.max_value_on_layer[layer.min(self.layer_count - 1)]
    }

                            pub fn is_layer_dependent(&self, class_i: usize, class_j: usize) -> bool {
        let compare_value = self.values[self.index(class_i, class_j, 0)];
        (1..self.layer_count).any(|l| self.values[self.index(class_i, class_j, l)] != compare_value)
    }

                                pub fn is_inner_layer_dependent(&self, class_i: usize, class_j: usize) -> bool {
        if self.layer_count <= 2 {
            return false;
        }
        let compare_value = self.values[self.index(class_i, class_j, 1)];
        (2..self.layer_count - 1)
            .any(|l| self.values[self.index(class_i, class_j, l)] != compare_value)
    }

                pub fn clearance_compensation_value(&self, clearance_class_index: usize, layer: usize) -> i32 {
        (self.get_value(clearance_class_index, clearance_class_index, layer, false) + 1) / 2
    }

                            pub fn append_class(&mut self, class_name: &str) -> bool {
        if self.get_no(class_name).is_some() {
            return false;
        }
        let old_class_count = self.get_class_count();
        let new_class_count = old_class_count + 1;

        let mut new_values = vec![0i32; new_class_count * new_class_count * self.layer_count];
        for j in 0..old_class_count {
            for i in 0..old_class_count {
                let src = (j * old_class_count + i) * self.layer_count;
                let dst = (j * new_class_count + i) * self.layer_count;
                new_values[dst..dst + self.layer_count]
                    .copy_from_slice(&self.values[src..src + self.layer_count]);
            }
        }
        self.values = new_values;
        self.row_max_value
            .resize(new_class_count * self.layer_count, 0);
        self.class_names.push(class_name.to_string());

        for i in 0..old_class_count {
            for j in 0..self.layer_count {
                let default_value = self.get_value(1, i, j, false);
                self.set_value(old_class_count, i, j, default_value);
                self.set_value(i, old_class_count, j, default_value);
            }
        }
        for j in 0..self.layer_count {
            let default_value = self.get_value(1, 1, j, false);
            self.set_value(old_class_count, old_class_count, j, default_value);
        }
        true
    }

                            pub fn remove_class(&mut self, index: usize) {
        let old_class_count = self.get_class_count();
        let new_class_count = old_class_count - 1;

        let mut new_values = vec![0i32; new_class_count * new_class_count * self.layer_count];
        let mut new_row_index = 0;
        for j in 0..old_class_count {
            if j == index {
                continue;
            }
            let mut new_column_index = 0;
            for i in 0..old_class_count {
                if i == index {
                    continue;
                }
                let src = (j * old_class_count + i) * self.layer_count;
                let dst = (new_row_index * new_class_count + new_column_index) * self.layer_count;
                new_values[dst..dst + self.layer_count]
                    .copy_from_slice(&self.values[src..src + self.layer_count]);
                new_column_index += 1;
            }
            new_row_index += 1;
        }
        self.values = new_values;
        self.class_names.remove(index);
        self.row_max_value = vec![0; new_class_count * self.layer_count];
    }

                                pub fn is_equal(&self, first: usize, second: usize) -> bool {
        if first == second {
            return true;
        }
        let class_count = self.get_class_count();
        if first >= class_count || second >= class_count {
            return false;
        }
        for i in 1..class_count {
            for layer in 0..self.layer_count {
                if self.values[self.index(i, first, layer)]
                    != self.values[self.index(i, second, layer)]
                {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structure::Layer;

                fn ls2() -> LayerStructure {
        LayerStructure::new(vec![Layer::new("Top", true), Layer::new("Bottom", true)])
    }

    fn ls3() -> LayerStructure {
        LayerStructure::new(vec![
            Layer::new("Top", true),
            Layer::new("Mid", true),
            Layer::new("Bottom", true),
        ])
    }

        fn dump(m: &ClearanceMatrix) -> Vec<Vec<Vec<i32>>> {
        (0..m.get_class_count())
            .map(|j| {
                (0..m.get_class_count())
                    .map(|i| {
                        (0..m.get_layer_count())
                            .map(|l| m.get_value(i, j, l, false))
                            .collect()
                    })
                    .collect()
            })
            .collect()
    }

    fn row_maxima(m: &ClearanceMatrix) -> Vec<Vec<i32>> {
        (0..m.get_class_count())
            .map(|j| {
                (0..m.get_layer_count())
                    .map(|l| m.max_value(j, l))
                    .collect()
            })
            .collect()
    }

    fn layer_maxima(m: &ClearanceMatrix) -> Vec<i32> {
        (0..m.get_layer_count())
            .map(|l| m.max_value_on_layer(l))
            .collect()
    }

    #[test]
    fn default_instance_initialises_only_classes_above_zero() {
        let m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        assert_eq!(m.get_class_count(), 2);
        assert_eq!(m.get_layer_count(), 2);
        assert_eq!(m.get_name(0), Some("null"));
        assert_eq!(m.get_name(1), Some("default"));
        assert_eq!(m.get_name(9), None);
        assert_eq!(
            dump(&m),
            vec![vec![vec![0, 0], vec![0, 0]], vec![vec![0, 0], vec![20, 20]],]
        );
        assert_eq!(row_maxima(&m), vec![vec![0, 0], vec![20, 20]]);
        assert_eq!(layer_maxima(&m), vec![20, 20]);
    }

    #[test]
    fn get_no_is_case_insensitive() {
        let m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        assert_eq!(m.get_no("default"), Some(1));
        assert_eq!(m.get_no("DEFAULT"), Some(1));
        assert_eq!(m.get_no("null"), Some(0));
        assert_eq!(m.get_no("x"), None);
    }

    #[test]
    fn clearance_compensation_value_is_half_the_self_clearance_rounded_up() {
        let m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        assert_eq!(m.clearance_compensation_value(1, 0), 10);
        assert_eq!(m.clearance_compensation_value(0, 0), 0);
    }

    #[test]
    fn out_of_range_arguments_answer_like_java() {
        let m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        assert_eq!(m.get_value(5, 0, 0, false), 0);
        assert_eq!(m.get_value(0, 0, 9, true), 0);
        assert_eq!(m.get_value(1, 1, 0, true), 36);
        assert_eq!(m.max_value(99, 99), 20);
        assert_eq!(m.max_value_on_layer(99), 20);
    }

    #[test]
    fn set_value_on_all_layers_then_set_default_value_on_one_layer() {
        let mut m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        m.set_value_on_all_layers(1, 1, 7);
        m.set_default_value_on_layer(1, 33);
        assert_eq!(
            dump(&m),
            vec![vec![vec![0, 0], vec![0, 0]], vec![vec![0, 0], vec![8, 34]]]
        );
        assert_eq!(row_maxima(&m), vec![vec![0, 0], vec![20, 34]]);
        assert_eq!(layer_maxima(&m), vec![20, 34]);
    }

    #[test]
    fn set_inner_value_touches_only_the_inner_layers() {
        let mut m = ClearanceMatrix::get_default_instance(&ls3(), 20);
        m.set_inner_value(1, 1, 44);
        assert_eq!(
            dump(&m),
            vec![
                vec![vec![0, 0, 0], vec![0, 0, 0]],
                vec![vec![0, 0, 0], vec![20, 44, 20]],
            ]
        );
        assert_eq!(layer_maxima(&m), vec![20, 44, 20]);
        assert!(!m.is_inner_layer_dependent(1, 1));
        assert!(m.is_layer_dependent(1, 1));

        let two = ClearanceMatrix::get_default_instance(&ls2(), 20);
        assert!(!two.is_inner_layer_dependent(1, 1));
    }

    #[test]
    fn append_class_copies_the_default_class_values() {
        let mut m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        m.set_value(1, 1, 0, 30);
        assert_eq!(row_maxima(&m), vec![vec![0, 0], vec![30, 20]]);

        assert!(m.append_class("power"));
        assert_eq!(m.get_class_count(), 3);
        assert_eq!(m.get_name(2), Some("power"));
        assert_eq!(
            dump(&m),
            vec![
                vec![vec![0, 0], vec![0, 0], vec![0, 0]],
                vec![vec![0, 0], vec![30, 20], vec![30, 20]],
                vec![vec![0, 0], vec![30, 20], vec![30, 20]],
            ]
        );
        assert_eq!(row_maxima(&m), vec![vec![0, 0], vec![30, 20], vec![30, 20]]);
        assert_eq!(layer_maxima(&m), vec![30, 20]);

        assert!(!m.append_class("POWER"));
        assert_eq!(m.get_class_count(), 3);

        assert!(m.append_class("other"));
        assert_eq!(
            dump(&m),
            vec![
                vec![vec![0, 0], vec![0, 0], vec![0, 0], vec![0, 0]],
                vec![vec![0, 0], vec![30, 20], vec![30, 20], vec![30, 20]],
                vec![vec![0, 0], vec![30, 20], vec![30, 20], vec![30, 20]],
                vec![vec![0, 0], vec![30, 20], vec![30, 20], vec![30, 20]],
            ]
        );
    }

    #[test]
    fn remove_class_drops_the_row_and_column_and_loses_every_row_maximum() {
        let mut m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        m.append_class("power");
        m.set_value(2, 2, 0, 50);
        m.set_value(1, 2, 0, 44);
        assert_eq!(
            dump(&m),
            vec![
                vec![vec![0, 0], vec![0, 0], vec![0, 0]],
                vec![vec![0, 0], vec![20, 20], vec![20, 20]],
                vec![vec![0, 0], vec![44, 20], vec![50, 20]],
            ]
        );
        assert_eq!(row_maxima(&m), vec![vec![0, 0], vec![20, 20], vec![50, 20]]);

        m.remove_class(1);

        assert_eq!(m.get_class_count(), 2);
        assert_eq!(m.get_name(0), Some("null"));
        assert_eq!(m.get_name(1), Some("power"));
        assert_eq!(
            dump(&m),
            vec![vec![vec![0, 0], vec![0, 0]], vec![vec![0, 0], vec![50, 20]]]
        );
        assert_eq!(row_maxima(&m), vec![vec![0, 0], vec![0, 0]]);
        assert_eq!(layer_maxima(&m), vec![50, 20]);
    }

    #[test]
    fn is_equal_skips_class_zero_and_short_circuits_on_identity() {
        let mut m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        m.append_class("power");
        assert!(m.is_equal(1, 2));
        assert!(!m.is_equal(0, 1));
        assert!(m.is_equal(3, 3));
        assert!(!m.is_equal(0, 3));

        m.set_value(1, 2, 0, 99);
        assert!(!m.is_equal(1, 2));
    }

    #[test]
    fn append_class_to_a_one_class_matrix_reads_the_still_empty_class_one() {
        let mut m = ClearanceMatrix::new(1, &ls2(), &["default"]);
        m.set_value(0, 0, 0, 12);
        assert_eq!(dump(&m), vec![vec![vec![12, 0]]]);
        assert_eq!(row_maxima(&m), vec![vec![12, 0]]);

        assert!(m.append_class("new"));
        assert_eq!(m.get_class_count(), 2);
        assert_eq!(m.get_name(1), Some("new"));
        assert_eq!(
            dump(&m),
            vec![vec![vec![12, 0], vec![0, 0]], vec![vec![0, 0], vec![0, 0]],]
        );
        assert_eq!(row_maxima(&m), vec![vec![12, 0], vec![0, 0]]);
        assert_eq!(layer_maxima(&m), vec![12, 0]);
    }

    #[test]
    fn remove_class_zero_shifts_the_default_class_down() {
        let mut m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        m.remove_class(0);
        assert_eq!(m.get_class_count(), 1);
        assert_eq!(m.get_name(0), Some("default"));
        assert_eq!(dump(&m), vec![vec![vec![20, 20]]]);
        assert_eq!(row_maxima(&m), vec![vec![0, 0]]);
        assert_eq!(layer_maxima(&m), vec![20, 20]);
    }

    #[test]
    fn default_instance_rounds_an_odd_default_value_up() {
        let m = ClearanceMatrix::get_default_instance(&ls2(), 21);
        assert_eq!(
            dump(&m),
            vec![vec![vec![0, 0], vec![0, 0]], vec![vec![0, 0], vec![22, 22]]]
        );
        assert_eq!(layer_maxima(&m), vec![22, 22]);
    }

    #[test]
    #[should_panic(expected = "clearance class 2 out of range")]
    fn set_value_panics_on_an_out_of_range_class_i_like_java() {
        let mut m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        m.set_value(2, 0, 0, 42);
    }

    #[test]
    #[should_panic(expected = "clearance class 2 out of range")]
    fn set_value_panics_on_an_out_of_range_class_j_like_java() {
        let mut m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        m.set_value(0, 2, 0, 42);
    }

    #[test]
    #[should_panic(expected = "layer 2 out of range")]
    fn set_value_panics_on_an_out_of_range_layer_like_java() {
        let mut m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        m.set_value(0, 0, 2, 42);
    }

    #[test]
    #[should_panic(expected = "clearance class 2 out of range")]
    fn is_layer_dependent_panics_on_an_out_of_range_class_like_java() {
        let m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        let _ = m.is_layer_dependent(2, 0);
    }

    #[test]
    fn the_neighbours_of_an_out_of_range_write_are_untouched() {
        let mut m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        let before = dump(&m);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            m.set_value(2, 0, 0, 42);
        }));
        assert!(result.is_err());
        assert_eq!(dump(&m), before);
        assert_eq!(
            dump(&m),
            vec![vec![vec![0, 0], vec![0, 0]], vec![vec![0, 0], vec![20, 20]]]
        );
    }

    #[test]
    fn out_of_range_is_inner_layer_dependent_returns_false_before_indexing() {
        let m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        assert!(!m.is_inner_layer_dependent(2, 0));
    }

    #[test]
    fn constructor_clamps_the_class_count_up_to_one() {
        let m = ClearanceMatrix::new(0, &ls2(), &["only"]);
        assert_eq!(m.get_class_count(), 1);
        assert_eq!(m.get_name(0), Some("only"));
    }
}
