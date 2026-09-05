# Plan 7 Task 14c — the equal tree-op-sequence transcripts (quirk #229's proof medium)

Committed evidence for the `p6t17b-bisect.patch` **level 8** ledgers in this directory: the
before/after of quirk #229, whose fix is `Board::undo_from_snapshot`
(`crates/fr-board/src/board/snapshot.rs`). Re-run the recipe below to check it has not
regressed — it is the only witness that can see the replay's *order*.

Task 14b's instrument, unchanged, on both sides. Levels: `P7T8B_IDS=1 P7T14B_MAT=1 P7T14B_FP=1`.
The Java half is **level 8** of `scripts/differential/java/p6t17b-bisect.patch`, compiled with
JDK 25 against `../freerouting/build/libs/freerouting-current-executable.jar`; the port half is
`scripts/differential/rust/target/release/p7t8`. Normalisation before diffing is Task 14b's: strip
`cls=` from the Java `MAT` lines, and keep only the `MAT`/`TREEFP`/`ITEMSEP` records.

Recipe (the full one is in the patch header and in `task-14b-report.md` §5):

```sh
grep -E '^(MAT|TREEFP|ITEMSEP)' j.err | sed 's/ cls=-*[0-9]*//' > j.mat
grep -E '^(MAT|TREEFP|ITEMSEP)' r.err                          > r.mat
diff j.mat r.mat
```

## 1. `Issue558-dev-board.dsn item 1 3` — the whole stream

| | Java | port |
|---|---|---|
| `MAT`/`TREEFP`/`ITEMSEP` records | **134 910** | **134 910** |
| `diff j.mat r.mat` | **empty** (exit 0) | |

Task 14b measured **54 Java-only `MAT` ops** here, between the last common op
(`MAT ins n=657 bb=(1193147,-582170,1211464,-578170)`, the tail of item `n=1`, the run's first
`improved=false` item) and `ITEMSEP n=2 id=10126`. Those 54 ops are the undo's side effects. They
are now present on **both** sides, in the same order, byte for byte. The window is lines
128 752-128 807 of both files and is reproduced in full below — one column, because the two files
are identical there and everywhere else.

```
MAT ins n=657 bb=(1193147,-582170,1211464,-578170)
MAT rem n=658 bb=(1192017,-583300,1196147,-579170)
MAT rem n=658 bb=(1191017,-584300,1197147,-578170)
MAT rem n=658 bb=(1191017,-584300,1197147,-578170)
MAT rem n=657 bb=(1142000,-737650,1251474,-628176)
MAT rem n=656 bb=(1249474,-630176,1251474,-620180)
MAT rem n=655 bb=(1208464,-622180,1251474,-579170)
MAT rem n=654 bb=(1194147,-581170,1210464,-579170)
MAT rem n=657 bb=(1141000,-738650,1252474,-627176)
MAT rem n=656 bb=(1248474,-631176,1252474,-619180)
MAT rem n=655 bb=(1207464,-623180,1252474,-578170)
MAT rem n=654 bb=(1193147,-582170,1211464,-578170)
MAT rem n=657 bb=(1141000,-738650,1252474,-627176)
MAT rem n=656 bb=(1248474,-631176,1252474,-619180)
MAT rem n=655 bb=(1207464,-623180,1252474,-578170)
MAT rem n=654 bb=(1193147,-582170,1211464,-578170)
MAT rem n=653 bb=(1181500,-583300,1194017,-581300)
MAT rem n=653 bb=(1180500,-584300,1195017,-580300)
MAT rem n=653 bb=(1180500,-584300,1195017,-580300)
MAT rem n=652 bb=(1192147,-583170,1198147,-577170)
MAT rem n=651 bb=(1192147,-583170,1198147,-577170)
MAT rem n=652 bb=(1191147,-584170,1199147,-576170)
MAT rem n=651 bb=(1191147,-584170,1199147,-576170)
MAT rem n=652 bb=(1191147,-584170,1199147,-576170)
MAT rem n=651 bb=(1191147,-584170,1199147,-576170)
MAT ins n=650 bb=(1165568,-588715,1172983,-581300)
MAT ins n=651 bb=(1165568,-640712,1167568,-586715)
MAT ins n=652 bb=(1162395,-643885,1167568,-638712)
MAT ins n=653 bb=(1162395,-648867,1164395,-641885)
MAT ins n=654 bb=(1162395,-652040,1167568,-646867)
MAT ins n=655 bb=(1165568,-653834,1167568,-650040)
MAT ins n=656 bb=(1161551,-657851,1167568,-651834)
MAT ins n=657 bb=(1161551,-718099,1163551,-655851)
MAT ins n=658 bb=(1142000,-737650,1163551,-716099)
MAT ins n=650 bb=(1164568,-589715,1173983,-580300)
MAT ins n=651 bb=(1164568,-641712,1168568,-585715)
MAT ins n=652 bb=(1161395,-644885,1168568,-637712)
MAT ins n=653 bb=(1161395,-649867,1165395,-640885)
MAT ins n=654 bb=(1161395,-653040,1168568,-645867)
MAT ins n=655 bb=(1164568,-654834,1168568,-649040)
MAT ins n=656 bb=(1160551,-658851,1168568,-650834)
MAT ins n=657 bb=(1160551,-719099,1164551,-654851)
MAT ins n=658 bb=(1141000,-738650,1164551,-715099)
MAT ins n=650 bb=(1164568,-589715,1173983,-580300)
MAT ins n=651 bb=(1164568,-641712,1168568,-585715)
MAT ins n=652 bb=(1161395,-644885,1168568,-637712)
MAT ins n=653 bb=(1161395,-649867,1165395,-640885)
MAT ins n=654 bb=(1161395,-653040,1168568,-645867)
MAT ins n=655 bb=(1164568,-654834,1168568,-649040)
MAT ins n=656 bb=(1160551,-658851,1168568,-650834)
MAT ins n=657 bb=(1160551,-719099,1164551,-654851)
MAT ins n=658 bb=(1141000,-738650,1164551,-715099)
MAT ins n=659 bb=(1170983,-583300,1183500,-581300)
MAT ins n=659 bb=(1169983,-584300,1184500,-580300)
MAT ins n=659 bb=(1169983,-584300,1184500,-580300)
ITEMSEP n=2 id=10126
```

## 2. `Issue026-J2_reference.dsn item 2 6` — the whole stream

| | Java | port |
|---|---|---|
| `MAT`/`TREEFP`/`ITEMSEP` records | **89 629** | **89 629** |
| `diff j2.mat r2.mat` | **empty** (exit 0) | |

Task 14b measured **84 Java-only `MAT` ops** at the tail of item `n=3` (`RESULT n=3 itemId=3754
improved=false`), immediately before `ITEMSEP n=4`. The `ITEMSEP` offsets in both files are
identical — `n=0` at 25 899, `n=1` at 36 729, `n=2` at 47 204, `n=3` at 57 967, `n=4` at 68 869,
`n=5` at 79 062 — so the divergence Task 14b reported at line 68 784 of 89 629 is gone. The first
40 of the 84 ops:

```
MAT rem n=215 bb=(1082850,-906750,1102887,-904250)
MAT rem n=215 bb=(1081849,-907751,1103888,-903249)
MAT rem n=214 bb=(1100387,-937187,1111053,-926521)
MAT rem n=213 bb=(1100387,-929021,1102887,-904250)
MAT rem n=214 bb=(1099386,-938188,1112054,-925520)
MAT rem n=213 bb=(1099386,-930022,1103888,-903249)
MAT rem n=212 bb=(1108553,-938782,1112648,-934687)
MAT rem n=211 bb=(1110148,-938782,1299032,-936282)
MAT rem n=210 bb=(1296532,-941561,1301811,-936282)
MAT rem n=212 bb=(1107552,-939783,1113649,-933686)
MAT rem n=211 bb=(1109147,-939783,1300033,-935281)
MAT rem n=210 bb=(1295531,-942562,1302812,-935281)
MAT rem n=209 bb=(1325153,-915719,1327653,-905553)
MAT rem n=208 bb=(1299311,-941561,1327653,-913219)
MAT rem n=209 bb=(1324152,-916720,1328654,-904552)
MAT rem n=208 bb=(1298310,-942562,1328654,-912218)
MAT rem n=207 bb=(1299500,-908500,1314269,-906000)
MAT rem n=207 bb=(1298499,-909501,1315270,-904999)
MAT rem n=206 bb=(1312216,-908053,1327653,-905553)
MAT rem n=205 bb=(1311769,-908500,1314716,-905553)
MAT rem n=206 bb=(1311215,-909054,1328654,-904552)
MAT rem n=205 bb=(1310768,-909501,1315717,-904552)
MAT rem n=204 bb=(1322403,-910803,1330403,-902803)
MAT rem n=203 bb=(1322403,-910803,1330403,-902803)
MAT rem n=204 bb=(1321402,-911804,1331404,-901802)
MAT rem n=203 bb=(1321402,-911804,1331404,-901802)
MAT rem n=202 bb=(1296561,-944311,1304561,-936311)
MAT rem n=201 bb=(1296561,-944311,1304561,-936311)
MAT rem n=202 bb=(1295560,-945312,1305562,-935310)
MAT rem n=201 bb=(1295560,-945312,1305562,-935310)
MAT rem n=200 bb=(1311769,-928035,1314269,-914000)
MAT rem n=199 bb=(1305541,-934263,1314269,-925535)
MAT rem n=198 bb=(1117622,-934263,1308041,-931763)
MAT rem n=197 bb=(1100109,-934263,1120122,-914250)
MAT rem n=196 bb=(1082850,-916750,1102609,-914250)
MAT rem n=200 bb=(1310768,-929036,1315270,-912999)
MAT rem n=199 bb=(1304540,-935264,1315270,-924534)
MAT rem n=198 bb=(1116621,-935264,1309042,-930762)
MAT rem n=197 bb=(1099108,-935264,1121123,-913249)
MAT rem n=196 bb=(1081849,-917751,1103610,-913249)
```

## 3. What the streams say

`MAT ins|rem n=<leafCount before> bb=(…)` is one line per `MinAreaTree` leaf insertion and removal
on **every** tree, so the sequence above is exactly `applyUndoRedoSideEffects`' replay:
`searchTreeManager.remove` over the cancelled items (`BasicBoard.java:1262`) and
`searchTreeManager.insert` over the restored ones (`:1276`), each item's shapes hitting all three
trees (default plus the two compensated ones — the removals come in runs of three or four for that
reason). `TREEFP n=<nodes> h=<fnv1a>` is the pre-order fingerprint of the whole autoroute tree,
taken once per `completeShape`: Task 14b's first divergence was `n=1299 h=280ed9f078230cef` (Java)
against `n=1299 h=989ad3553fee2c88` (port) — same leaves, different topology. Every `TREEFP` line
in both files now agrees on both fields, which is the statement quirk #229 needed.
