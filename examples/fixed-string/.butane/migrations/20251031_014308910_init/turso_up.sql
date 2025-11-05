CREATE TABLE Config ("key" TEXT NOT NULL PRIMARY KEY, "value" TEXT NOT NULL, description TEXT);
CREATE TABLE "Order" (
    "id" INTEGER NOT NULL PRIMARY KEY,
    order_number TEXT NOT NULL,
    "user" INTEGER NOT NULL,
    product TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    "status" TEXT NOT NULL,
    FOREIGN KEY ("user") REFERENCES "User"("id"),
    FOREIGN KEY (product) REFERENCES Product(sku)
);
CREATE TABLE Product (
    sku TEXT NOT NULL PRIMARY KEY,
    "name" TEXT NOT NULL,
    category TEXT NOT NULL,
    price_cents INTEGER NOT NULL,
    in_stock INTEGER NOT NULL
);
CREATE TABLE "User" (
    "id" INTEGER NOT NULL PRIMARY KEY,
    username TEXT NOT NULL,
    email TEXT NOT NULL,
    display_name TEXT,
    "status" TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS butane_migrations ("name" TEXT NOT NULL PRIMARY KEY);
