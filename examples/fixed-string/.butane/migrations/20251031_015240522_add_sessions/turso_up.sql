CREATE TABLE "Session" (
    session_id TEXT NOT NULL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    ip_address TEXT NOT NULL,
    user_agent TEXT NOT NULL,
    "status" TEXT NOT NULL,
    device_fingerprint TEXT
);
