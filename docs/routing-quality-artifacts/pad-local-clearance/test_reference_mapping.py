import unittest
from reference_mapping import remap_references


class ReferenceMappingTest(unittest.TestCase):
    def test_duplicate_reference_uses_pad_position_and_footprint_uuid(self):
        records = [dict(component='REF**', pad='1', x_um=104000, y_um=85500,
                        copper_um=1000, mask_margin_um=20)]
        pads = [dict(component='REF**', pad='1', x_um=104000, y_um=54000, uuid='a'),
                dict(component='REF**', pad='1', x_um=104000, y_um=85500, uuid='b')]
        mapped, changes, unmatched = remap_references(records, pads, {'a': 'REF**', 'b': 'REF**_3'})
        self.assertEqual(mapped, [dict(records[0], component='REF**_3')])
        self.assertEqual(records[0]['component'], 'REF**')
        self.assertEqual(len(changes), 1)
        self.assertEqual(unmatched, [])

    def test_coincident_layers_with_one_destination_are_not_ambiguous(self):
        record = dict(component='U1', pad='1', x_um=10, y_um=20, copper_um=300)
        pad = dict(record, uuid='a')
        mapped, changes, unmatched = remap_references([record], [pad, pad], {'a': 'U1'})
        self.assertEqual(mapped, [record])
        self.assertEqual((changes, unmatched), ([], []))

    def test_coincident_footprints_cannot_choose_different_destinations(self):
        record = dict(component='REF**', pad='1', x_um=10, y_um=20)
        with self.assertRaisesRegex(ValueError, 'ambiguous'):
            remap_references([record], [dict(record, uuid='a'), dict(record, uuid='b')],
                             {'a': 'REF**', 'b': 'REF**_2'})

    def test_missing_destination_uuid_fails_instead_of_guessing(self):
        record = dict(component='REF**', pad='1', x_um=10, y_um=20)
        with self.assertRaisesRegex(ValueError, 'missing'):
            remap_references([record], [dict(record, uuid='a')], {})

    def test_unmatched_metadata_is_preserved_and_reported(self):
        record = dict(component='Q2', pad='5', x_um=10, y_um=20)
        mapped, changes, unmatched = remap_references([record], [], {})
        self.assertEqual(mapped, [record])
        self.assertEqual(changes, [])
        self.assertEqual(unmatched, [0])


if __name__ == '__main__':
    unittest.main()
