pub fn normalize_email(email: &str) -> String {
    let email = email.trim().to_lowercase();

    let Some((local, domain)) = email.split_once('@') else {
        return email;
    };

    // Strip +tag subaddressing (Gmail, Outlook, Fastmail, etc.)
    let local = match local.split_once('+') {
        Some((base, _)) => base,
        None => local,
    };

    // Strip dots from local part for Gmail/Googlemail
    let local = if domain == "gmail.com" || domain == "googlemail.com" {
        local.replace('.', "")
    } else {
        local.to_string()
    };

    format!("{}@{}", local, domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_normalization() {
        assert_eq!(normalize_email("Brian@Gmail.com"), "brian@gmail.com");
    }

    #[test]
    fn test_strip_subaddress() {
        assert_eq!(normalize_email("brian+spam@gmail.com"), "brian@gmail.com");
    }

    #[test]
    fn test_strip_dots_gmail() {
        assert_eq!(normalize_email("b.ri.an@gmail.com"), "brian@gmail.com");
    }

    #[test]
    fn test_dots_preserved_non_gmail() {
        assert_eq!(normalize_email("b.rian@outlook.com"), "b.rian@outlook.com");
    }

    #[test]
    fn test_combined() {
        assert_eq!(normalize_email("  B.Rian+news@Gmail.com  "), "brian@gmail.com");
    }
}
