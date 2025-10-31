CREATE TABLE Post_likes_Many (
`owner` INT NOT NULL,
has VARCHAR(255) NOT NULL
);
ALTER TABLE Post_likes_Many ADD CONSTRAINT Post_likes_Many_owner_fkey FOREIGN KEY (`owner`) REFERENCES Post(`id`);
ALTER TABLE Post_likes_Many ADD CONSTRAINT Post_likes_Many_has_fkey FOREIGN KEY (has) REFERENCES `User`(`id`);
