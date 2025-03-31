ALTER TABLE Post_tags_Many DROP CONSTRAINT Post_tags_Many_owner_fkey;
ALTER TABLE Post_tags_Many DROP CONSTRAINT Post_tags_Many_has_fkey;
DROP TABLE Post_tags_Many;
DROP TABLE "Tag";
ALTER TABLE Post ADD COLUMN tags JSONB NOT NULL DEFAULT null;
