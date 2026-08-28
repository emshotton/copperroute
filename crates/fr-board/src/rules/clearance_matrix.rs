//! The clearance matrix: required spacing between clearance classes, per layer.
//!
//! Java: `rules/ClearanceMatrix.java`.

use crate::structure::LayerStructure;

use super::equals_ignore_case;

/// `ClearanceMatrix.clearance_safety_margin` (ClearanceMatrix.java:17): the constant added to
/// every clearance value when [`ClearanceMatrix::get_value`] is asked for the safety margin.
pub const CLEARANCE_SAFETY_MARGIN: i32 = 16;

/// Port of `ClearanceMatrix` (`rules/ClearanceMatrix.java`): an NxN matrix describing the
/// spacing restrictions between N clearance classes on a fixed set of layers.
///
/// Java stores the matrix as `Row[] row`, each `Row` holding a `MatrixEntry[] column` and an
/// `int[] maxValue`, each `MatrixEntry` holding an `int[] layer`
/// (ClearanceMatrix.java:376-453). The port flattens all three into dense vectors; the private
/// `Row`/`MatrixEntry` classes have no other purpose (their only escape hatch, `getRow`, is not
/// ported — see below).
///
/// **Indexing is J-then-I.** Java's accessor is
/// `row[classJ].column[classI].layer[layer]` (ClearanceMatrix.java:163, and identically in
/// `setValue` at :101-102): the *second* class argument selects the row and the *first* selects
/// the column. `setValue` writes exactly one entry, so the matrix is only symmetric if a caller
/// writes both orders — which is why the DSN reader always does
/// (`io/specctra/parser/Structure.java:847-848` walks `i`, then `j >= i`, setting both).
///
/// not ported: `ClearanceMatrix.getRow` (ClearanceMatrix.java:254-260) returns the private
/// `Row` inner class, which exists only to be handed to `ItemInfoPrinter`; its three call sites
/// are `NetClass.printInfo` (NetClass.java:224), `ViaInfo.printInfo` (ViaInfo.java:99) and
/// `Item.printClearanceInfo` (Item.java:1113), all GUI-only info panels. No
/// `board`/`autoroute`/`drc`/`io` code calls it.
///
/// not ported: `ClearanceMatrix.printInfo` / `Row.printInfo` (ClearanceMatrix.java:392-418) —
/// `ItemInfoPrinter.Printable`, GUI only (see the module docs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClearanceMatrix {
    /// `layerStructure.layers.length` (ClearanceMatrix.java:18 holds the whole `LayerStructure`,
    /// but every use of it is `layers.length` except the two `printInfo` bodies).
    layer_count: usize,
    /// The `Row.name`s (ClearanceMatrix.java:378). `classCount` is `class_names.len()`: Java
    /// keeps `classCount` and `row.length` in lock-step everywhere except transiently inside
    /// `appendClass`/`removeClass`.
    class_names: Vec<String>,
    /// `row[j].column[i].layer[l]`, flattened as `(j * class_count + i) * layer_count + l`.
    values: Vec<i32>,
    /// `row[j].maxValue[l]`, flattened as `j * layer_count + l`.
    row_max_value: Vec<i32>,
    /// `maxValueOnLayer` (ClearanceMatrix.java:19).
    max_value_on_layer: Vec<i32>,
}

impl ClearanceMatrix {
    /// Port of the `ClearanceMatrix(int, LayerStructure, String[])` constructor
    /// (ClearanceMatrix.java:30-38).
    ///
    /// Java clamps `classCount` up to 1 (:31) and then reads `names[i]` for every class, so a
    /// `names` shorter than the clamped count throws `ArrayIndexOutOfBoundsException`; the port
    /// panics on the same slice index.
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

    /// Port of `ClearanceMatrix.getDefaultInstance` (ClearanceMatrix.java:44-52): the 2-class
    /// matrix `["null", "default"]` initialised with `default_value`.
    ///
    /// Note the class *named* `"null"` at index 0 — Java writes the four-character string, not a
    /// null reference (ClearanceMatrix.java:47).
    pub fn get_default_instance(
        layer_structure: &LayerStructure,
        default_value: i32,
    ) -> ClearanceMatrix {
        let mut result = ClearanceMatrix::new(2, layer_structure, &["null", "default"]);
        result.set_default_value(default_value);
        result
    }

    /// Port of `ClearanceMatrix.getClassCount` (ClearanceMatrix.java:263-265).
    pub fn get_class_count(&self) -> usize {
        self.class_names.len()
    }

    /// Port of `ClearanceMatrix.getLayerCount` (ClearanceMatrix.java:268-270).
    pub fn get_layer_count(&self) -> usize {
        self.layer_count
    }

    /// Flattened index of `row[class_j].column[class_i].layer[layer]`.
    fn index(&self, class_i: usize, class_j: usize, layer: usize) -> usize {
        (class_j * self.get_class_count() + class_i) * self.layer_count + layer
    }

    /// Port of `ClearanceMatrix.getNo` (ClearanceMatrix.java:58-65): the index of the clearance
    /// class named `name`, ignoring case, or `None` (Java: `-1`).
    pub fn get_no(&self, name: &str) -> Option<usize> {
        self.class_names
            .iter()
            .position(|n| equals_ignore_case(n, name))
    }

    /// Port of `ClearanceMatrix.getName` (ClearanceMatrix.java:68-74). Java warns and returns
    /// `null` for an out-of-range index; ported as `None`.
    pub fn get_name(&self, clearance_class_index: usize) -> Option<&str> {
        self.class_names
            .get(clearance_class_index)
            .map(String::as_str)
    }

    /// Port of `ClearanceMatrix.setDefaultValue(int)` (ClearanceMatrix.java:77-81): sets the
    /// value of all clearance classes with number >= 1 on every layer.
    pub fn set_default_value(&mut self, value: i32) {
        for layer in 0..self.layer_count {
            self.set_default_value_on_layer(layer, value);
        }
    }

    /// Port of `ClearanceMatrix.setDefaultValue(int, int)` (ClearanceMatrix.java:84-90).
    ///
    /// Both loops start at 1, so class 0 — the "no clearance" class,
    /// `BoardRules.clearanceClassNone()` — keeps its zeros.
    pub fn set_default_value_on_layer(&mut self, layer: usize, value: i32) {
        for i in 1..self.get_class_count() {
            for j in 1..self.get_class_count() {
                self.set_value(i, j, layer, value);
            }
        }
    }

    /// Port of `ClearanceMatrix.setValue(int, int, int)` (ClearanceMatrix.java:93-97): sets one
    /// entry on all layers.
    pub fn set_value_on_all_layers(&mut self, class_i: usize, class_j: usize, value: i32) {
        for layer in 0..self.layer_count {
            self.set_value(class_i, class_j, layer, value);
        }
    }

    /// Port of `ClearanceMatrix.setValue(int, int, int, int)` (ClearanceMatrix.java:100-118).
    ///
    /// The value is clamped to be non-negative and rounded *up* to an even number, except that
    /// `i32::MAX` is rounded *down* to `i32::MAX - 1` to dodge the overflow
    /// (ClearanceMatrix.java:106-113). Both the row maximum and the per-layer maximum are
    /// running maxima that this method only ever raises — lowering an entry never lowers them
    /// (pinned by `set_value_matches_java_test`).
    ///
    /// Writes `row[class_j].column[class_i]`: see the J-then-I note on the type.
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

    /// Port of `ClearanceMatrix.setInnerValue` (ClearanceMatrix.java:121-125): sets one entry on
    /// every layer except the first and the last. A board with fewer than three layers has no
    /// inner layers and the loop body never runs.
    pub fn set_inner_value(&mut self, class_i: usize, class_j: usize, value: i32) {
        for layer in 1..self.layer_count.saturating_sub(1) {
            self.set_value(class_i, class_j, layer, value);
        }
    }

    /// Port of `ClearanceMatrix.getValue` (ClearanceMatrix.java:131-201): the required spacing
    /// between `class_i` and `class_j` on `layer`, always an even integer.
    ///
    /// Reads `row[class_j].column[class_i]` (ClearanceMatrix.java:163) — the J-then-I indexing.
    /// Out-of-range arguments return 0 exactly as Java does (:133-161, where the `FRLogger.trace`
    /// call is diagnostic only).
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

    /// Port of `ClearanceMatrix.maxValue(int, int)` (ClearanceMatrix.java:207-213): the maximal
    /// required spacing of `class_i` to all other clearance classes on `layer`.
    ///
    /// Java clamps both arguments into range (`Math.max`/`Math.min`); with `usize` arguments only
    /// the upper clamp can bite. Reads `row[class_i].maxValue` — the row maxima are maintained on
    /// the J axis by [`Self::set_value`], and reading them at index `class_i` is what makes this
    /// "the maximum over everything paired *with* `class_i`".
    pub fn max_value(&self, class_i: usize, layer: usize) -> i32 {
        let i = class_i.min(self.get_class_count() - 1);
        let layer_index = layer.min(self.layer_count - 1);
        self.row_max_value[i * self.layer_count + layer_index]
    }

    /// Port of `ClearanceMatrix.maxValue(int)` (ClearanceMatrix.java:216-220): the maximum
    /// clearance value on `layer`, clamped into range as Java does.
    pub fn max_value_on_layer(&self, layer: usize) -> i32 {
        self.max_value_on_layer[layer.min(self.layer_count - 1)]
    }

    /// Port of `ClearanceMatrix.isLayerDependent` (ClearanceMatrix.java:226-234): true if the
    /// entry `(class_i, class_j)` differs across layers.
    ///
    /// Java indexes `row[classJ].column[classI]` unchecked, so an out-of-range class throws;
    /// the port panics on the same slice index.
    pub fn is_layer_dependent(&self, class_i: usize, class_j: usize) -> bool {
        let compare_value = self.values[self.index(class_i, class_j, 0)];
        (1..self.layer_count).any(|l| self.values[self.index(class_i, class_j, l)] != compare_value)
    }

    /// Port of `ClearanceMatrix.isInnerLayerDependent` (ClearanceMatrix.java:240-251): as
    /// [`Self::is_layer_dependent`], but over the inner layers only, and `false` for a board with
    /// two layers or fewer.
    ///
    /// Note Java's asymmetric loop bounds: it compares against layer 1 and scans `2 ..
    /// layers.length - 1`, so the *last* layer is excluded but layer 1 is not — correct for a
    /// stack whose outer layers are the first and last.
    pub fn is_inner_layer_dependent(&self, class_i: usize, class_j: usize) -> bool {
        if self.layer_count <= 2 {
            return false;
        }
        let compare_value = self.values[self.index(class_i, class_j, 1)];
        (2..self.layer_count - 1)
            .any(|l| self.values[self.index(class_i, class_j, l)] != compare_value)
    }

    /// Port of `ClearanceMatrix.clearanceCompensationValue` (ClearanceMatrix.java:273-275):
    /// `(getValue(c, c, layer, false) + 1) / 2`, i.e. half the class's clearance to itself,
    /// rounded up. Java's `/` is integer division and the value is never negative.
    pub fn clearance_compensation_value(&self, clearance_class_index: usize, layer: usize) -> i32 {
        (self.get_value(clearance_class_index, clearance_class_index, layer, false) + 1) / 2
    }

    /// Port of `ClearanceMatrix.appendClass` (ClearanceMatrix.java:281-322): appends a clearance
    /// class initialised from the values of the default class (class 1). Returns false — leaving
    /// the matrix untouched — if a class with that name already exists.
    ///
    /// The initialisation loops are transcribed in Java's order because they read through
    /// `getValue` while writing through `setValue`, which also raises the running maxima.
    pub fn append_class(&mut self, class_name: &str) -> bool {
        if self.get_no(class_name).is_some() {
            return false;
        }
        let old_class_count = self.get_class_count();
        let new_class_count = old_class_count + 1;

        // Grow `values` from an `old x old` block matrix to a `new x new` one, zero-filling the
        // appended column of every old row and the whole appended row. This is Java's
        // `System.arraycopy` of each old row's columns plus the two fresh `MatrixEntry`s
        // (ClearanceMatrix.java:288-303).
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
        // Java carries each old row's `maxValue` array across by reference
        // (ClearanceMatrix.java:295) and gives the appended row a fresh zeroed one (:388).
        self.row_max_value
            .resize(new_class_count * self.layer_count, 0);
        self.class_names.push(class_name.to_string());

        // Set the new matrix elements to default values (ClearanceMatrix.java:309-320).
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

    /// Port of `ClearanceMatrix.removeClass` (ClearanceMatrix.java:325-352): drops the row and
    /// the column with the given index.
    ///
    /// Java's method is package-private and reached only through
    /// `BoardRules.removeClearanceClass` (BoardRules.java:347); it is `pub` here because the
    /// caller lives in a different Rust module.
    //
    // Java bug: rebuilding the rows with `new Row(currentOldRow.name)` (ClearanceMatrix.java:338)
    // hands every surviving row a **freshly zeroed** `maxValue` array — the old maxima are
    // dropped on the floor — while `maxValueOnLayer` is not recomputed at all and keeps maxima
    // that may have belonged to the removed class. So after a removal `maxValue(classI, layer)`
    // answers 0 for every class until the next `setValue`, and `maxValue(layer)` can answer too
    // high. Reproduced here; see docs/java-quirks.md (#36).
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
        // Java bug (see above): every row maximum is reset to zero, `max_value_on_layer` is left
        // stale.
        self.row_max_value = vec![0; new_class_count * self.layer_count];
    }

    /// Port of `ClearanceMatrix.isEqual` (ClearanceMatrix.java:358-373): true if the classes
    /// `first` and `second` have the same clearance values against every class.
    ///
    /// Java's loop starts at column 1, so the clearance to class 0 (the "no clearance" class) is
    /// deliberately not compared, and an out-of-range index answers `false` unless both indices
    /// are the identical out-of-range value (:359-364, the `first == second` shortcut comes
    /// first).
    pub fn is_equal(&self, first: usize, second: usize) -> bool {
        if first == second {
            return true;
        }
        let class_count = self.get_class_count();
        if first >= class_count || second >= class_count {
            return false;
        }
        for i in 1..class_count {
            // `MatrixEntry.equals` compares all layer values (ClearanceMatrix.java:434-441).
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

    /// Every expected value in this module was produced by running the Java `ClearanceMatrix`
    /// class body verbatim (with `LayerStructure` stubbed and the `FRLogger` calls removed) under
    /// JDK 23 and dumping the whole matrix — see the task report for the harness.
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

    /// All `get_value` results as `[[per layer]; class_i]; class_j`.
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
        // Java: `defaultInstance(20)` on the 2-layer stack.
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
        // ClearanceMatrix.java:60 uses `equalsIgnoreCase`.
        let m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        assert_eq!(m.get_no("default"), Some(1));
        assert_eq!(m.get_no("DEFAULT"), Some(1));
        // Class 0 is *named* "null" (ClearanceMatrix.java:47), it is not a null reference.
        assert_eq!(m.get_no("null"), Some(0));
        assert_eq!(m.get_no("x"), None);
    }

    #[test]
    fn clearance_compensation_value_is_half_the_self_clearance_rounded_up() {
        // ClearanceMatrix.java:274.
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
        // maxValue clamps its arguments into range (ClearanceMatrix.java:208-211,217-218).
        assert_eq!(m.max_value(99, 99), 20);
        assert_eq!(m.max_value_on_layer(99), 20);
    }

    #[test]
    fn set_value_on_all_layers_then_set_default_value_on_one_layer() {
        // Java: setValue(1, 1, 7) then setDefaultValue(1, 33) on defaultInstance(20).
        let mut m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        m.set_value_on_all_layers(1, 1, 7);
        m.set_default_value_on_layer(1, 33);
        assert_eq!(
            dump(&m),
            vec![vec![vec![0, 0], vec![0, 0]], vec![vec![0, 0], vec![8, 34]]]
        );
        // Layer 0's row maximum keeps the 20 it reached before the value dropped to 8.
        assert_eq!(row_maxima(&m), vec![vec![0, 0], vec![20, 34]]);
        assert_eq!(layer_maxima(&m), vec![20, 34]);
    }

    #[test]
    fn set_inner_value_touches_only_the_inner_layers() {
        // Java: setInnerValue(1, 1, 44) on a 3-layer defaultInstance(20).
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
        // isInnerLayerDependent compares layer 1 against layers 2..layerCount-1, an empty range
        // on a 3-layer board, so it is false even though the value *is* layer dependent.
        assert!(!m.is_inner_layer_dependent(1, 1));
        assert!(m.is_layer_dependent(1, 1));

        // Two layers or fewer: always false (ClearanceMatrix.java:241-243).
        let two = ClearanceMatrix::get_default_instance(&ls2(), 20);
        assert!(!two.is_inner_layer_dependent(1, 1));
    }

    #[test]
    fn append_class_copies_the_default_class_values() {
        // Java: defaultInstance(20); setValue(1, 1, 0, 30); appendClass("power").
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

        // The duplicate check is case insensitive and changes nothing (ClearanceMatrix.java:282).
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
        // Java: defaultInstance(20); appendClass("power"); setValue(2,2,0,50);
        // setValue(1,2,0,44); removeClass(1).
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
        // Java bug (docs/java-quirks.md #36): every row maximum is silently reset to zero, while
        // the per-layer maximum keeps the 50 that the surviving class happens to still have and
        // would keep it even if that class had been the one removed.
        assert_eq!(row_maxima(&m), vec![vec![0, 0], vec![0, 0]]);
        assert_eq!(layer_maxima(&m), vec![50, 20]);
    }

    #[test]
    fn is_equal_skips_class_zero_and_short_circuits_on_identity() {
        // Java: defaultInstance(20); appendClass("power").
        let mut m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        m.append_class("power");
        assert!(m.is_equal(1, 2));
        assert!(!m.is_equal(0, 1));
        // `first == second` wins even for an out-of-range index (ClearanceMatrix.java:359-361).
        assert!(m.is_equal(3, 3));
        assert!(!m.is_equal(0, 3));

        m.set_value(1, 2, 0, 99);
        assert!(!m.is_equal(1, 2));
    }

    #[test]
    fn append_class_to_a_one_class_matrix_reads_the_still_empty_class_one() {
        // Java: new ClearanceMatrix(1, ls2, {"default"}); setValue(0, 0, 0, 12);
        // appendClass("new"). `appendClass` seeds the new entries from `getValue(1, ...)`, and
        // class 1 is the class it just created, so everything comes out zero
        // (ClearanceMatrix.java:311,318).
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
        // Java: defaultInstance(20); removeClass(0).
        let mut m = ClearanceMatrix::get_default_instance(&ls2(), 20);
        m.remove_class(0);
        assert_eq!(m.get_class_count(), 1);
        assert_eq!(m.get_name(0), Some("default"));
        assert_eq!(dump(&m), vec![vec![vec![20, 20]]]);
        // Java bug #36 again: the row maximum is lost, the layer maximum is not.
        assert_eq!(row_maxima(&m), vec![vec![0, 0]]);
        assert_eq!(layer_maxima(&m), vec![20, 20]);
    }

    #[test]
    fn default_instance_rounds_an_odd_default_value_up() {
        // Java: defaultInstance(21) -> 22 everywhere (ClearanceMatrix.java:107-112).
        let m = ClearanceMatrix::get_default_instance(&ls2(), 21);
        assert_eq!(
            dump(&m),
            vec![vec![vec![0, 0], vec![0, 0]], vec![vec![0, 0], vec![22, 22]]]
        );
        assert_eq!(layer_maxima(&m), vec![22, 22]);
    }

    #[test]
    fn constructor_clamps_the_class_count_up_to_one() {
        // ClearanceMatrix.java:31: `Math.max(classCount, 1)`.
        let m = ClearanceMatrix::new(0, &ls2(), &["only"]);
        assert_eq!(m.get_class_count(), 1);
        assert_eq!(m.get_name(0), Some("only"));
    }
}
