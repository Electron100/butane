ALTER TABLE Post ADD FOREIGN KEY (blog) REFERENCES Blog("id");
ALTER TABLE Post_tags_Many ADD FOREIGN KEY (has) REFERENCES "Tag"("tag");
ALTER TABLE Post_tags_Many ADD FOREIGN KEY ("owner") REFERENCES Post("id");
