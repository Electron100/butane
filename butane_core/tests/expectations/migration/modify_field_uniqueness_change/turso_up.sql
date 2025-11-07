CREATE TABLE Foo__butane_tmp ("id" INTEGER NOT NULL PRIMARY KEY, bar INTEGER NOT NULL);
CREATE UNIQUE INDEX Foo__butane_tmp_bar_unique_idx ON Foo__butane_tmp (bar);
INSERT INTO Foo__butane_tmp SELECT "id", bar FROM Foo;
DROP TABLE Foo;
ALTER TABLE Foo__butane_tmp RENAME TO Foo;
