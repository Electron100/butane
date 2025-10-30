ALTER TABLE Post_tags_Many DROP FOREIGN KEY Post_tags_Many_owner_fkey;
ALTER TABLE Post_tags_Many DROP FOREIGN KEY Post_tags_Many_has_fkey;
DROP TABLE Post_tags_Many;
DROP TABLE `Tag`;
ALTER TABLE Post ADD COLUMN tags JSON NOT NULL DEFAULT ('[]');
