PRAGMA defer_foreign_keys = ON;
CREATE TABLE Foo__butane_tmp (
"id" INTEGER NOT NULL PRIMARY KEY,
bar INTEGER NOT NULL UNIQUE
) STRICT;
INSERT INTO Foo__butane_tmp SELECT "id", bar FROM Foo;
DROP TABLE Foo;
ALTER TABLE Foo__butane_tmp RENAME TO Foo;
PRAGMA defer_foreign_keys = OFF;
