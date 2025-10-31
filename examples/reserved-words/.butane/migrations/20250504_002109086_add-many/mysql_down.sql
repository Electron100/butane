ALTER TABLE Post_likes_Many DROP FOREIGN KEY Post_likes_Many_owner_fkey;
ALTER TABLE Post_likes_Many DROP FOREIGN KEY Post_likes_Many_has_fkey;
DROP TABLE Post_likes_Many;
