CREATE TABLE Blog (
"id" INTEGER NOT NULL PRIMARY KEY,
"name" TEXT NOT NULL
) STRICT;
CREATE TABLE Post (
"id" INTEGER NOT NULL PRIMARY KEY,
title TEXT NOT NULL,
body TEXT NOT NULL,
published INTEGER NOT NULL,
blog INTEGER NOT NULL,
byline TEXT
) STRICT;
CREATE TABLE Post_tags_Many (
"owner" INTEGER NOT NULL,
has TEXT NOT NULL
) STRICT;
CREATE TABLE Tag (
"tag" TEXT NOT NULL PRIMARY KEY
) STRICT;
CREATE TABLE IF NOT EXISTS butane_migrations (
"name" TEXT NOT NULL PRIMARY KEY
) STRICT;
