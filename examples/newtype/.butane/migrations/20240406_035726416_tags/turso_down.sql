CREATE TABLE Post_tags_Many (
    "owner" BLOB NOT NULL,
    has TEXT NOT NULL,
    FOREIGN KEY ("owner") REFERENCES Post("id"),
    FOREIGN KEY (has) REFERENCES "Tag"("tag")
);
CREATE TABLE "Tag" ("tag" TEXT NOT NULL PRIMARY KEY);
ALTER TABLE Post DROP COLUMN tags;
