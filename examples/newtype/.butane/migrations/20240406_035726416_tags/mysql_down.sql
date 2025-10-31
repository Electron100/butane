CREATE TABLE Post_tags_Many (
`owner` VARBINARY(255) NOT NULL,
has VARCHAR(255) NOT NULL
);
CREATE TABLE `Tag` (
`tag` VARCHAR(255) NOT NULL PRIMARY KEY
);
ALTER TABLE Post DROP COLUMN tags;
ALTER TABLE Post_tags_Many ADD CONSTRAINT Post_tags_Many_owner_fkey FOREIGN KEY (`owner`) REFERENCES Post(`id`);
ALTER TABLE Post_tags_Many ADD CONSTRAINT Post_tags_Many_has_fkey FOREIGN KEY (has) REFERENCES `Tag`(`tag`);
