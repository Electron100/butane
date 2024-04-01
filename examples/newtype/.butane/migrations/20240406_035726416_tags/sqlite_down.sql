CREATE TABLE Post_tags_Many (
owner BLOB NOT NULL,
has TEXT NOT NULL,
FOREIGN KEY (owner) REFERENCES Post(id)
FOREIGN KEY (has) REFERENCES Tag(tag)
);
CREATE TABLE Tag (
tag TEXT NOT NULL PRIMARY KEY
);
CREATE TABLE Post__butane_tmp (
id BLOB NOT NULL PRIMARY KEY,
title TEXT NOT NULL,
body TEXT NOT NULL,
published INTEGER NOT NULL,
blog BLOB NOT NULL,
byline TEXT,
likes INTEGER NOT NULL,
FOREIGN KEY (blog) REFERENCES Blog(id)
);
INSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;
DROP TABLE Post;
ALTER TABLE Post__butane_tmp RENAME TO Post;
