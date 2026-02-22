-- Create transactions table
CREATE TABLE IF NOT EXISTS transactions (
    id TEXT NOT NULL PRIMARY KEY,
    buyer_id TEXT NOT NULL,
    seller_id TEXT NOT NULL,
    listing_id TEXT NOT NULL,
    amount INTEGER NOT NULL, -- in smallest currency unit (e.g., piconeros)
    status TEXT NOT NULL DEFAULT 'pending', -- pending, confirmed, completed, cancelled, disputed
    escrow_release_time DATETIME, -- when funds should be released from escrow
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for transactions
CREATE INDEX IF NOT EXISTS idx_transactions_buyer_id ON transactions(buyer_id);
CREATE INDEX IF NOT EXISTS idx_transactions_seller_id ON transactions(seller_id);
CREATE INDEX IF NOT EXISTS idx_transactions_listing_id ON transactions(listing_id);
CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status);
CREATE INDEX IF NOT EXISTS idx_transactions_escrow_release ON transactions(escrow_release_time);
CREATE INDEX IF NOT EXISTS idx_transactions_created_at ON transactions(created_at);

-- Create composite indexes for common queries
CREATE INDEX IF NOT EXISTS idx_transactions_buyer_created ON transactions(buyer_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_seller_created ON transactions(seller_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_status_escrow ON transactions(status, escrow_release_time);