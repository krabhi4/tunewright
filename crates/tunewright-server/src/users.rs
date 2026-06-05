use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    SuperAdmin,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
}

/// User info without password hash, for API responses.
#[derive(Debug, Clone, Serialize)]
pub struct UserPublic {
    pub id: String,
    pub username: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
}

impl From<&User> for UserPublic {
    fn from(u: &User) -> Self {
        Self {
            id: u.id.clone(),
            username: u.username.clone(),
            role: u.role,
            created_at: u.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invite {
    pub token: String,
    pub created_by: String,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct UserStore {
    users: Vec<User>,
    invites: Vec<Invite>,
}

#[derive(Debug, Clone)]
pub struct UserManager {
    store: Arc<Mutex<UserStore>>,
    write_lock: Arc<Mutex<()>>,
    path: PathBuf,
}

impl UserManager {
    /// Load users from disk. Panics if the file exists but contains invalid JSON
    /// to prevent silent user data loss (which would allow anyone to re-run setup).
    pub fn load(path: PathBuf) -> Self {
        let store = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) if contents.trim().is_empty() => UserStore::default(),
                Ok(contents) => match serde_json::from_str(&contents) {
                    Ok(store) => store,
                    Err(e) => {
                        tracing::error!(
                            "FATAL: users.json at {:?} exists but contains invalid JSON: {}. \
                             Refusing to start to prevent data loss. \
                             Fix or remove the file manually.",
                            path, e
                        );
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    tracing::error!(
                        "FATAL: Cannot read users file {:?}: {}. \
                         Refusing to start to prevent data loss.",
                        path, e
                    );
                    std::process::exit(1);
                }
            }
        } else {
            UserStore::default()
        };
        Self {
            store: Arc::new(Mutex::new(store)),
            write_lock: Arc::new(Mutex::new(())),
            path,
        }
    }

    fn save_store_data(&self, store: &UserStore) -> Result<(), &'static str> {
        let json = serde_json::to_string_pretty(store).map_err(|e| {
            tracing::error!("Failed to serialize users: {}", e);
            "Failed to save user data"
        })?;

        use std::io::Write;
        let tmp_path = self.path.with_extension("json.tmp");

        let mut file = std::fs::File::create(&tmp_path).map_err(|e| {
            tracing::error!("Failed to create users temp file: {}", e);
            "Failed to save user data"
        })?;
        file.write_all(json.as_bytes()).map_err(|e| {
            tracing::error!("Failed to write users temp file: {}", e);
            "Failed to save user data"
        })?;
        file.sync_all().map_err(|e| {
            tracing::error!("Failed to fsync users temp file: {}", e);
            "Failed to save user data"
        })?;
        drop(file);

        std::fs::rename(&tmp_path, &self.path).map_err(|e| {
            tracing::error!("Failed to rename users file: {}", e);
            "Failed to save user data"
        })?;

        if let Some(parent) = self.path.parent() {
            if let Ok(dir) = std::fs::File::open(parent) {
                let _ = dir.sync_all();
            }
        }

        Ok(())
    }

    pub fn has_users(&self) -> bool {
        let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
        !store.users.is_empty()
    }

    pub fn find_by_username(&self, username: &str) -> Option<User> {
        let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
        let normalized = username.trim().to_lowercase();
        store
            .users
            .iter()
            .find(|u| u.username == normalized)
            .cloned()
        // Mutex is dropped here
    }

    /// Atomically create the first user. Returns Err if users already exist or save fails.
    pub fn add_first_user(
        &self,
        username: &str,
        password_hash: String,
    ) -> Result<User, &'static str> {
        let _write_guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());

        let mut cloned_store = {
            let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
            if !store.users.is_empty() {
                return Err("Setup already completed");
            }
            store.clone()
        };

        let user = User {
            id: Uuid::new_v4().to_string(),
            username: username.trim().to_lowercase(),
            password_hash,
            role: Role::SuperAdmin,
            created_at: Utc::now(),
        };
        cloned_store.users.push(user.clone());

        self.save_store_data(&cloned_store)?;

        // Success, now update the actual store
        {
            let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
            if !store.users.is_empty() {
                return Err("Setup already completed");
            }
            *store = cloned_store;
        }

        Ok(user)
    }

    /// Remove a user. Returns Err if trying to remove the last super_admin or save fails.
    pub fn remove_user(&self, id: &str) -> Result<bool, &'static str> {
        let _write_guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());

        let mut cloned_store = {
            let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
            store.clone()
        };

        let idx = match cloned_store.users.iter().position(|u| u.id == id) {
            None => return Ok(false),
            Some(i) => i,
        };
        if cloned_store.users[idx].role == Role::SuperAdmin {
            let super_admin_count = cloned_store
                .users
                .iter()
                .filter(|u| u.role == Role::SuperAdmin)
                .count();
            if super_admin_count <= 1 {
                return Err("Cannot delete the last super admin");
            }
        }
        cloned_store.users.remove(idx);

        self.save_store_data(&cloned_store)?;

        // Success, now update the actual store
        {
            let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
            *store = cloned_store;
        }

        Ok(true)
    }

    pub fn list_users(&self) -> Vec<UserPublic> {
        let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
        store.users.iter().map(UserPublic::from).collect()
    }

    pub fn create_invite(&self, created_by: &str) -> Result<Invite, &'static str> {
        let _write_guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());

        let mut cloned_store = {
            let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
            store.clone()
        };

        let invite = Invite {
            token: Uuid::new_v4().to_string(),
            created_by: created_by.to_string(),
            expires_at: Utc::now() + Duration::hours(48),
            used: false,
        };
        cloned_store.invites.push(invite.clone());

        self.save_store_data(&cloned_store)?;

        // Success, now update the actual store
        {
            let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
            *store = cloned_store;
        }

        Ok(invite)
    }

    /// Atomically consume invite and create user. Returns Err on invalid invite, duplicate username, or save failure.
    pub fn register_with_invite(
        &self,
        token: &str,
        username: &str,
        password_hash: String,
    ) -> Result<User, &'static str> {
        let _write_guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());

        let mut cloned_store = {
            let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
            store.clone()
        };

        let normalized = username.trim().to_lowercase();

        if cloned_store.users.iter().any(|u| u.username == normalized) {
            return Err("Username already taken");
        }

        let invite_idx = cloned_store
            .invites
            .iter()
            .position(|i| i.token == token)
            .ok_or("Invalid or expired invite")?;
        if cloned_store.invites[invite_idx].used
            || cloned_store.invites[invite_idx].expires_at < Utc::now()
        {
            return Err("Invalid or expired invite");
        }
        cloned_store.invites[invite_idx].used = true;

        let user = User {
            id: Uuid::new_v4().to_string(),
            username: normalized,
            password_hash,
            role: Role::Admin,
            created_at: Utc::now(),
        };
        cloned_store.users.push(user.clone());

        self.save_store_data(&cloned_store)?;

        // Success, now update the actual store
        {
            let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
            *store = cloned_store;
        }

        Ok(user)
    }

    pub fn list_invites(&self) -> Vec<Invite> {
        let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
        let now = Utc::now();
        store
            .invites
            .iter()
            .filter(|i| !i.used && i.expires_at > now)
            .cloned()
            .collect()
    }

    pub fn delete_invite(&self, token: &str) -> Result<bool, &'static str> {
        let _write_guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());

        let mut cloned_store = {
            let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
            store.clone()
        };

        let idx = match cloned_store.invites.iter().position(|i| i.token == token) {
            None => return Ok(false),
            Some(i) => i,
        };
        cloned_store.invites.remove(idx);

        self.save_store_data(&cloned_store)?;

        // Success, now update the actual store
        {
            let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
            *store = cloned_store;
        }

        Ok(true)
    }
}

/// Hash a password with argon2. Call from spawn_blocking.
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let mut salt_bytes = [0u8; 16];
    rand::fill(&mut salt_bytes);
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|_| {
        argon2::password_hash::Error::SaltInvalid(
            argon2::password_hash::errors::InvalidValue::Malformed,
        )
    })?;
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

/// Verify a password against an argon2 hash. Call from spawn_blocking.
pub fn verify_password(password: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_non_existent() {
        let path = std::env::temp_dir().join(format!("users_test_{}.json", uuid::Uuid::new_v4()));
        let manager = UserManager::load(path.clone());
        assert!(!manager.has_users());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_add_first_user_and_has_users() {
        let path = std::env::temp_dir().join(format!("users_test_{}.json", uuid::Uuid::new_v4()));
        let manager = UserManager::load(path.clone());
        assert!(!manager.has_users());

        let user = manager
            .add_first_user("admin", "some_hash".to_string())
            .unwrap();
        assert_eq!(user.username, "admin");
        assert_eq!(user.role, Role::SuperAdmin);
        assert!(manager.has_users());

        // Setup already completed
        assert!(manager.add_first_user("other", "hash".to_string()).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_invite_flow() {
        let path = std::env::temp_dir().join(format!("users_test_{}.json", uuid::Uuid::new_v4()));
        let manager = UserManager::load(path.clone());
        // Create an invite
        let invite = manager.create_invite("admin").unwrap();
        assert_eq!(invite.created_by, "admin");
        assert!(!invite.used);

        // Register user with invite
        let user = manager
            .register_with_invite(&invite.token, "invited_user", "password_hash".to_string())
            .unwrap();
        assert_eq!(user.username, "invited_user");
        assert_eq!(user.role, Role::Admin);

        // Try using the same invite again
        assert!(manager
            .register_with_invite(&invite.token, "another_user", "password_hash".to_string())
            .is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_remove_user() {
        let path = std::env::temp_dir().join(format!("users_test_{}.json", uuid::Uuid::new_v4()));
        let manager = UserManager::load(path.clone());
        let admin = manager
            .add_first_user("admin", "some_hash".to_string())
            .unwrap();

        // Cannot delete last super admin
        assert!(manager.remove_user(&admin.id).is_err());

        // Create invite and register an admin user
        let invite = manager.create_invite("admin").unwrap();
        let user = manager
            .register_with_invite(&invite.token, "regular_user", "hash".to_string())
            .unwrap();

        assert!(manager.remove_user(&user.id).unwrap());
        // User should not be found anymore
        assert!(manager.find_by_username("regular_user").is_none());
        let _ = std::fs::remove_file(&path);
    }
}
