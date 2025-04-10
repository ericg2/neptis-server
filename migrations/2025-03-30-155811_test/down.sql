-- This file should undo anything in `up.sql`
-- Drop the sessions table
DROP TABLE IF EXISTS sessions;

-- Drop the users table
DROP TABLE IF EXISTS users;

-- Drop the custom type EncodedHashType
DROP TYPE IF EXISTS "EncodedHashType";