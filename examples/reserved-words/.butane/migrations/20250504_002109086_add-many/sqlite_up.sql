CREATE TABLE Post_likes_Many (
"owner" INTEGER NOT NULL,
has TEXT NOT NULL,
FOREIGN KEY ("owner") REFERENCES Post("id")
FOREIGN KEY (has) REFERENCES "User"("id")
) STRICT;
