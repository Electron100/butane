CREATE TABLE Post (
    "id" INTEGER NOT NULL PRIMARY KEY,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    published INTEGER NOT NULL,
    byline TEXT,
    FOREIGN KEY (byline) REFERENCES "User"("id")
) STRICT;
