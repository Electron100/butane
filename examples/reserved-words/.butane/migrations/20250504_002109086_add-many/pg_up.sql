CREATE TABLE Post_likes_Many (
"owner" INTEGER NOT NULL,
has TEXT NOT NULL
);
ALTER TABLE Post_likes_Many ADD FOREIGN KEY ("owner") REFERENCES Post("id");
ALTER TABLE Post_likes_Many ADD FOREIGN KEY (has) REFERENCES "User"("id");
