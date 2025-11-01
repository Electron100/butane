ALTER TABLE "Order" DROP CONSTRAINT "Order_user_fkey";
ALTER TABLE "Order" DROP CONSTRAINT "Order_product_fkey";
DROP TABLE Config;
DROP TABLE "Order";
DROP TABLE Product;
DROP TABLE "User";
