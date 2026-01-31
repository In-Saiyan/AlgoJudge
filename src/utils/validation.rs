//! Input validation utilities

use crate::constants;

/// Validate username format
pub fn validate_username(username: &str) -> Result<(), &'static str> {
    if username.len() < 3 {
        return Err("Username must be at least 3 characters");
    }
    if username.len() > 32 {
        return Err("Username must be at most 32 characters");
    }
    if !username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Username can only contain letters, numbers, underscores, and hyphens");
    }
    if !username.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false) {
        return Err("Username must start with a letter");
    }
    Ok(())
}

/// Validate email format (basic validation)
pub fn validate_email(email: &str) -> Result<(), &'static str> {
    if !email.contains('@') {
        return Err("Invalid email format");
    }
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return Err("Invalid email format");
    }
    if parts[0].is_empty() || parts[1].is_empty() {
        return Err("Invalid email format");
    }
    if !parts[1].contains('.') {
        return Err("Invalid email domain");
    }
    Ok(())
}

/// Validate password strength
pub fn validate_password(password: &str) -> Result<(), &'static str> {
    if password.len() < 8 {
        return Err("Password must be at least 8 characters");
    }
    if password.len() > 128 {
        return Err("Password must be at most 128 characters");
    }
    if !password.chars().any(|c| c.is_lowercase()) {
        return Err("Password must contain at least one lowercase letter");
    }
    if !password.chars().any(|c| c.is_uppercase()) {
        return Err("Password must contain at least one uppercase letter");
    }
    if !password.chars().any(|c| c.is_numeric()) {
        return Err("Password must contain at least one number");
    }
    Ok(())
}

/// Validate programming language
pub fn validate_language(language: &str) -> Result<(), &'static str> {
    if constants::languages::ALL.contains(&language) {
        Ok(())
    } else {
        Err("Unsupported programming language")
    }
}

/// Validate contest scoring mode
pub fn validate_scoring_mode(mode: &str) -> Result<(), &'static str> {
    if constants::scoring_modes::ALL.contains(&mode) {
        Ok(())
    } else {
        Err("Invalid scoring mode")
    }
}

/// Validate user role
pub fn validate_role(role: &str) -> Result<(), &'static str> {
    if constants::roles::ALL.contains(&role) {
        Ok(())
    } else {
        Err("Invalid role")
    }
}

/// Validate contest visibility
pub fn validate_visibility(visibility: &str) -> Result<(), &'static str> {
    match visibility {
        "public" | "private" | "hidden" => Ok(()),
        _ => Err("Invalid visibility setting"),
    }
}

/// Validate source code size
pub fn validate_source_code(code: &str) -> Result<(), &'static str> {
    if code.is_empty() {
        return Err("Source code cannot be empty");
    }
    if code.len() > 65536 {
        // 64KB
        return Err("Source code exceeds maximum size of 64KB");
    }
    Ok(())
}

/// Sanitize string input (remove control characters, trim whitespace)
pub fn sanitize_string(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect::<String>()
        .trim()
        .to_string()
}

/// Validate and sanitize problem title
pub fn validate_problem_title(title: &str) -> Result<String, &'static str> {
    let sanitized = sanitize_string(title);
    if sanitized.is_empty() {
        return Err("Problem title cannot be empty");
    }
    if sanitized.len() > 256 {
        return Err("Problem title must be at most 256 characters");
    }
    Ok(sanitized)
}

/// Validate time limit (in milliseconds)
pub fn validate_time_limit(ms: i32) -> Result<(), &'static str> {
    if ms < 100 {
        return Err("Time limit must be at least 100ms");
    }
    if ms > 30000 {
        return Err("Time limit must be at most 30 seconds");
    }
    Ok(())
}

/// Validate memory limit (in KB)
pub fn validate_memory_limit(kb: i32) -> Result<(), &'static str> {
    if kb < 1024 {
        return Err("Memory limit must be at least 1MB");
    }
    if kb > 1048576 {
        return Err("Memory limit must be at most 1GB");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_username() {
        assert!(validate_username("alice").is_ok());
        assert!(validate_username("Alice_123").is_ok());
        assert!(validate_username("ab").is_err()); // Too short
        assert!(validate_username("123abc").is_err()); // Starts with number
        assert!(validate_username("user@name").is_err()); // Invalid character
    }

    #[test]
    fn test_validate_email() {
        assert!(validate_email("user@example.com").is_ok());
        assert!(validate_email("invalid").is_err());
        assert!(validate_email("@example.com").is_err());
        assert!(validate_email("user@").is_err());
    }

    #[test]
    fn test_validate_password() {
        assert!(validate_password("Password123").is_ok());
        assert!(validate_password("short").is_err());
        assert!(validate_password("nouppercase123").is_err());
        assert!(validate_password("NOLOWERCASE123").is_err());
        assert!(validate_password("NoNumbers").is_err());
    }

    #[test]
    fn test_validate_language() {
        assert!(validate_language("c").is_ok());
        assert!(validate_language("cpp").is_ok());
        assert!(validate_language("rust").is_ok());
        assert!(validate_language("invalid").is_err());
    }
}
