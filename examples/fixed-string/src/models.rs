//! Models for the fixed-string example demonstrating ArrayString usage.

use arrayvec::ArrayString;
use butane::{model, AutoPk, ForeignKey};

/// User account with fixed-size string fields for performance.
#[model]
#[derive(Debug, Default, Clone)]
pub struct User {
    /// User ID.
    pub id: AutoPk<i64>,
    /// Username - limited to 32 characters for efficient storage.
    pub username: ArrayString<32>,
    /// Email address - limited to 255 characters (RFC standard).
    pub email: ArrayString<255>,
    /// Optional display name - limited to 64 characters.
    pub display_name: Option<ArrayString<64>>,
    /// User status: active, suspended, deleted, etc.
    pub status: ArrayString<16>,
}

impl User {
    /// Create a new user with the given username and email.
    pub fn new(username: &str, email: &str) -> Result<Self, arrayvec::CapacityError> {
        let mut user = User {
            id: AutoPk::uninitialized(),
            username: ArrayString::new(),
            email: ArrayString::new(),
            display_name: None,
            status: ArrayString::from("active").map_err(|_| arrayvec::CapacityError::new(()))?,
        };

        user.username.push_str(username);
        if user.username.len() != username.len() {
            return Err(arrayvec::CapacityError::new(()));
        }

        user.email.push_str(email);
        if user.email.len() != email.len() {
            return Err(arrayvec::CapacityError::new(()));
        }

        Ok(user)
    }

    /// Set the display name for this user.
    pub fn with_display_name(
        mut self,
        display_name: &str,
    ) -> Result<Self, arrayvec::CapacityError> {
        let name = ArrayString::from(display_name).map_err(|_| arrayvec::CapacityError::new(()))?;
        self.display_name = Some(name);
        Ok(self)
    }

    /// Set the user status.
    pub fn with_status(mut self, status: &str) -> Result<Self, arrayvec::CapacityError> {
        self.status = ArrayString::from(status).map_err(|_| arrayvec::CapacityError::new(()))?;
        Ok(self)
    }
}

/// Product catalog entry with fixed-size strings for inventory management.
#[model]
#[derive(Debug, Default, Clone)]
pub struct Product {
    /// Product SKU - used as primary key, limited to 32 characters.
    #[pk]
    pub sku: ArrayString<32>,
    /// Product name - limited to 128 characters.
    pub name: ArrayString<128>,
    /// Product category - limited to 64 characters.
    pub category: ArrayString<64>,
    /// Price in cents (to avoid floating point issues).
    pub price_cents: i64,
    /// Whether the product is currently in stock.
    pub in_stock: bool,
}

impl Product {
    /// Create a new product.
    pub fn new(
        sku: &str,
        name: &str,
        category: &str,
        price_cents: i64,
    ) -> Result<Self, arrayvec::CapacityError> {
        Ok(Product {
            sku: ArrayString::from(sku).map_err(|_| arrayvec::CapacityError::new(()))?,
            name: ArrayString::from(name).map_err(|_| arrayvec::CapacityError::new(()))?,
            category: ArrayString::from(category).map_err(|_| arrayvec::CapacityError::new(()))?,
            price_cents,
            in_stock: true,
        })
    }

    /// Set whether the product is in stock.
    pub fn set_in_stock(mut self, in_stock: bool) -> Self {
        self.in_stock = in_stock;
        self
    }
}

/// Order tracking with fixed-size identifiers.
#[model]
#[derive(Debug, Clone)]
pub struct Order {
    /// Order ID.
    pub id: AutoPk<i64>,
    /// Order number - customer-facing identifier, limited to 32 characters.
    pub order_number: ArrayString<32>,
    /// User who placed the order.
    pub user: ForeignKey<User>,
    /// Product being ordered.
    pub product: ForeignKey<Product>,
    /// Quantity ordered.
    pub quantity: i32,
    /// Order status: pending, shipped, delivered, cancelled.
    pub status: ArrayString<16>,
}

impl Order {
    /// Create a new order.
    pub fn new(
        order_number: &str,
        user: User,
        product: Product,
        quantity: i32,
    ) -> Result<Self, arrayvec::CapacityError> {
        Ok(Order {
            id: AutoPk::uninitialized(),
            order_number: ArrayString::from(order_number)
                .map_err(|_| arrayvec::CapacityError::new(()))?,
            user: user.into(),
            product: product.into(),
            quantity,
            status: ArrayString::from("pending").map_err(|_| arrayvec::CapacityError::new(()))?,
        })
    }

    /// Update the order status.
    pub fn with_status(mut self, status: &str) -> Result<Self, arrayvec::CapacityError> {
        self.status = ArrayString::from(status).map_err(|_| arrayvec::CapacityError::new(()))?;
        Ok(self)
    }
}

/// Configuration settings with fixed-size keys and values.
#[model]
#[derive(Debug, Default, Clone)]
pub struct Config {
    /// Configuration key - used as primary key.
    #[pk]
    pub key: ArrayString<64>,
    /// Configuration value.
    pub value: ArrayString<512>,
    /// Description of what this configuration does.
    pub description: Option<ArrayString<256>>,
}

impl Config {
    /// Create a new configuration entry.
    pub fn new(key: &str, value: &str) -> Result<Self, arrayvec::CapacityError> {
        Ok(Config {
            key: ArrayString::from(key).map_err(|_| arrayvec::CapacityError::new(()))?,
            value: ArrayString::from(value).map_err(|_| arrayvec::CapacityError::new(()))?,
            description: None,
        })
    }

    /// Add a description to the configuration.
    pub fn with_description(mut self, description: &str) -> Result<Self, arrayvec::CapacityError> {
        self.description =
            Some(ArrayString::from(description).map_err(|_| arrayvec::CapacityError::new(()))?);
        Ok(self)
    }
}

/// User session information with fixed-size string fields.
#[model]
#[derive(Debug, Default, Clone)]
pub struct Session {
    /// Session ID - used as primary key.
    #[pk]
    pub session_id: ArrayString<128>,
    /// User ID this session belongs to.
    pub user_id: i64,
    /// IP address from which the session was created.
    pub ip_address: ArrayString<45>, // IPv6 max length
    /// User agent string (truncated if necessary).
    pub user_agent: ArrayString<512>,
    /// Session status: active, expired, revoked.
    pub status: ArrayString<16>,
    /// Optional device fingerprint.
    pub device_fingerprint: Option<ArrayString<64>>,
}

impl Session {
    /// Create a new session.
    pub fn new(
        session_id: &str,
        user_id: i64,
        ip_address: &str,
        user_agent: &str,
    ) -> Result<Self, arrayvec::CapacityError> {
        Ok(Session {
            session_id: ArrayString::from(session_id)
                .map_err(|_| arrayvec::CapacityError::new(()))?,
            user_id,
            ip_address: ArrayString::from(ip_address)
                .map_err(|_| arrayvec::CapacityError::new(()))?,
            user_agent: ArrayString::from(user_agent)
                .map_err(|_| arrayvec::CapacityError::new(()))?,
            status: ArrayString::from("active").map_err(|_| arrayvec::CapacityError::new(()))?,
            device_fingerprint: None,
        })
    }

    /// Set the device fingerprint for this session.
    pub fn with_device_fingerprint(
        mut self,
        fingerprint: &str,
    ) -> Result<Self, arrayvec::CapacityError> {
        self.device_fingerprint =
            Some(ArrayString::from(fingerprint).map_err(|_| arrayvec::CapacityError::new(()))?);
        Ok(self)
    }

    /// Update the session status.
    pub fn with_status(mut self, status: &str) -> Result<Self, arrayvec::CapacityError> {
        self.status = ArrayString::from(status).map_err(|_| arrayvec::CapacityError::new(()))?;
        Ok(self)
    }
}
