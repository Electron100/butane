CREATE TABLE Post__butane_tmp (
"id" INTEGER NOT NULL PRIMARY KEY,
title TEXT NOT NULL,
body TEXT NOT NULL,
published INTEGER NOT NULL,
blog INTEGER NOT NULL,
byline TEXT,
likes INTEGER NOT NULL,
FOREIGN KEY (blog) REFERENCES Blog("id")
);
INSERT INTO Post__butane_tmp SELECT "id", title, body, published, blog, byline, likes FROM Post;
DROP TABLE Post;
ALTER TABLE Post__butane_tmp RENAME TO Post;
CREATE TABLE Post_tags_Many__butane_tmp (
"owner" INTEGER NOT NULL,
has TEXT NOT NULL,
FOREIGN KEY (has) REFERENCES "Tag"("tag")
);
INSERT INTO Post_tags_Many__butane_tmp SELECT "owner", has FROM Post_tags_Many;
DROP TABLE Post_tags_Many;
ALTER TABLE Post_tags_Many__butane_tmp RENAME TO Post_tags_Many;
CREATE TABLE Post_tags_Many__butane_tmp (
"owner" INTEGER NOT NULL,
has TEXT NOT NULL,
FOREIGN KEY ("owner") REFERENCES Post("id")
FOREIGN KEY (has) REFERENCES "Tag"("tag")
);
INSERT INTO Post_tags_Many__butane_tmp SELECT "owner", has FROM Post_tags_Many;
DROP TABLE Post_tags_Many;
ALTER TABLE Post_tags_Many__butane_tmp RENAME TO Post_tags_Many;
