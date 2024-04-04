CREATE TABLE Blog__butane_tmp (
id BIGSERIAL NOT NULL PRIMARY KEY,
"name" TEXT NOT NULL
);
INSERT INTO Blog__butane_tmp SELECT id, "name" FROM Blog;
DROP TABLE Blog;
ALTER TABLE Blog__butane_tmp RENAME TO Blog;
CREATE TABLE Blog__butane_tmp (
id BIGSERIAL NOT NULL PRIMARY KEY,
"name" TEXT NOT NULL
);
INSERT INTO Blog__butane_tmp SELECT id, "name" FROM Blog;
DROP TABLE Blog;
ALTER TABLE Blog__butane_tmp RENAME TO Blog;
CREATE TABLE Post__butane_tmp (
id SERIAL NOT NULL PRIMARY KEY,
title TEXT NOT NULL,
body TEXT NOT NULL,
published BOOLEAN NOT NULL,
blog BIGINT NOT NULL,
byline TEXT ,
likes INTEGER NOT NULL
);
INSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;
DROP TABLE Post;
ALTER TABLE Post__butane_tmp RENAME TO Post;
CREATE TABLE Post__butane_tmp (
id SERIAL NOT NULL PRIMARY KEY,
title TEXT NOT NULL,
body TEXT NOT NULL,
published BOOLEAN NOT NULL,
blog BIGINT NOT NULL,
byline TEXT ,
likes INTEGER NOT NULL
);
INSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;
DROP TABLE Post;
ALTER TABLE Post__butane_tmp RENAME TO Post;
CREATE TABLE Post__butane_tmp (
id SERIAL NOT NULL PRIMARY KEY,
title TEXT NOT NULL,
body TEXT NOT NULL,
published BOOLEAN NOT NULL,
blog BIGINT NOT NULL,
byline TEXT ,
likes INTEGER NOT NULL
);
INSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;
DROP TABLE Post;
ALTER TABLE Post__butane_tmp RENAME TO Post;
CREATE TABLE Post__butane_tmp (
id SERIAL NOT NULL PRIMARY KEY,
title TEXT NOT NULL,
body TEXT NOT NULL,
published BOOLEAN NOT NULL,
blog BIGINT NOT NULL,
byline TEXT ,
likes INTEGER NOT NULL
);
INSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;
DROP TABLE Post;
ALTER TABLE Post__butane_tmp RENAME TO Post;
CREATE TABLE Post__butane_tmp (
id SERIAL NOT NULL PRIMARY KEY,
title TEXT NOT NULL,
body TEXT NOT NULL,
published BOOLEAN NOT NULL,
blog BIGINT NOT NULL,
byline TEXT ,
likes INTEGER NOT NULL
);
INSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;
DROP TABLE Post;
ALTER TABLE Post__butane_tmp RENAME TO Post;
CREATE TABLE Post__butane_tmp (
id SERIAL NOT NULL PRIMARY KEY,
title TEXT NOT NULL,
body TEXT NOT NULL,
published BOOLEAN NOT NULL,
blog BIGINT NOT NULL,
byline TEXT ,
likes INTEGER NOT NULL
);
INSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;
DROP TABLE Post;
ALTER TABLE Post__butane_tmp RENAME TO Post;
CREATE TABLE Post__butane_tmp (
id SERIAL NOT NULL PRIMARY KEY,
title TEXT NOT NULL,
body TEXT NOT NULL,
published BOOLEAN NOT NULL,
blog BIGINT NOT NULL,
byline TEXT ,
likes INTEGER NOT NULL
);
INSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;
DROP TABLE Post;
ALTER TABLE Post__butane_tmp RENAME TO Post;
CREATE TABLE Post_tags_Many__butane_tmp (
owner INTEGER NOT NULL,
has TEXT NOT NULL
);
ALTER TABLE Post_tags_Many__butane_tmp ADD FOREIGN KEY (owner) REFERENCES Post(id);
INSERT INTO Post_tags_Many__butane_tmp SELECT owner, has FROM Post_tags_Many;
DROP TABLE Post_tags_Many;
ALTER TABLE Post_tags_Many__butane_tmp RENAME TO Post_tags_Many;
CREATE TABLE Post_tags_Many__butane_tmp (
owner INTEGER NOT NULL,
has TEXT NOT NULL
);
INSERT INTO Post_tags_Many__butane_tmp SELECT owner, has FROM Post_tags_Many;
DROP TABLE Post_tags_Many;
ALTER TABLE Post_tags_Many__butane_tmp RENAME TO Post_tags_Many;
CREATE TABLE Tag__butane_tmp (
tag TEXT NOT NULL PRIMARY KEY
);
INSERT INTO Tag__butane_tmp SELECT tag FROM Tag;
DROP TABLE Tag;
ALTER TABLE Tag__butane_tmp RENAME TO Tag;
