ALTER TABLE Post DROP CONSTRAINT Post_blog_fkey;
ALTER TABLE Post_tags_Many DROP CONSTRAINT Post_tags_Many_owner_fkey;
ALTER TABLE Post_tags_Many DROP CONSTRAINT Post_tags_Many_has_fkey;
DROP TABLE Blog;
DROP TABLE Post;
DROP TABLE Post_tags_Many;
DROP TABLE "Tag";
