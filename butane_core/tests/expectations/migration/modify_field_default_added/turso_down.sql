CREATE TABLE Foo__butane_tmp ("id" INTEGER NOT NULL PRIMARY KEY, bar TEXT NOT NULL);
INSERT INTO Foo__butane_tmp SELECT "id", bar FROM Foo;
DROP TABLE Foo;
ALTER TABLE Foo__butane_tmp RENAME TO Foo;
