CREATE TABLE Post__butane_tmp (
"id" INTEGER NOT NULL PRIMARY KEY,
title TEXT NOT NULL,
body TEXT NOT NULL,
published INTEGER NOT NULL,
blog INTEGER NOT NULL,
byline TEXT
) STRICT;
INSERT INTO Post__butane_tmp SELECT "id", title, body, published, blog, byline FROM Post;
DROP TABLE Post;
ALTER TABLE Post__butane_tmp RENAME TO Post;
