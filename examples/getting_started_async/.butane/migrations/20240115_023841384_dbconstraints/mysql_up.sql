ALTER TABLE Post ADD CONSTRAINT Post_blog_fkey FOREIGN KEY (blog) REFERENCES Blog(`id`);
ALTER TABLE Post_tags_Many ADD CONSTRAINT Post_tags_Many_has_fkey FOREIGN KEY (has) REFERENCES `Tag`(`tag`);
ALTER TABLE Post_tags_Many ADD CONSTRAINT Post_tags_Many_owner_fkey FOREIGN KEY (`owner`) REFERENCES Post(`id`);
