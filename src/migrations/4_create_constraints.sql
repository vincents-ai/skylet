-- Create foreign key constraints for data integrity
-- Note: SQLite doesn't support adding foreign key constraints to existing tables easily
-- so we include them here for documentation and future migrations

-- Users to Listings relationship
-- Each listing must reference a valid user (seller)
-- listings.seller_id -> users.id

-- Users to Transactions relationships  
-- Each transaction must reference valid users (buyer and seller)
-- transactions.buyer_id -> users.id
-- transactions.seller_id -> users.id

-- Listings to Transactions relationship
-- Each transaction must reference a valid listing
-- transactions.listing_id -> listings.id

-- Enable foreign key constraints for the database
PRAGMA foreign_keys = ON;

-- Create triggers for automatic updated_at timestamp on users
CREATE TRIGGER IF NOT EXISTS update_users_updated_at
    AFTER UPDATE ON users
BEGIN
    UPDATE users SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Create trigger to update listing status when transaction is completed
CREATE TRIGGER IF NOT EXISTS update_listing_on_transaction_complete
    AFTER UPDATE ON transactions
    WHEN NEW.status = 'completed' AND OLD.status != 'completed'
BEGIN
    UPDATE listings SET status = 'sold' WHERE id = NEW.listing_id;
END;