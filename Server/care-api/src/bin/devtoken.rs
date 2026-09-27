//! devtoken: prints an AUTH_MODE=dev access token for curl / API tests.
//!
//!   devtoken <user_uuid> [role ...]        e.g. devtoken 5eed0000-0000-4000-8000-000000000001 patient
//!
//! Uses DEV_JWT_SECRET and CLAIMS_NAMESPACE from the environment (same as care-api).

use care_api::auth::DevVerifier;
use std::time::Duration;

fn main() {
    care_api::install_crypto_provider();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(uid) = args.first().and_then(|a| a.parse::<uuid::Uuid>().ok()) else {
        eprintln!("usage: devtoken <user_uuid> [role ...]");
        std::process::exit(2);
    };
    let roles: Vec<&str> = if args.len() > 1 { args[1..].iter().map(String::as_str).collect() } else { vec!["patient"] };
    let secret = std::env::var("DEV_JWT_SECRET").unwrap_or_else(|_| "healoo-dev-secret-change-me".into());
    let ns = std::env::var("CLAIMS_NAMESPACE").unwrap_or_else(|_| "https://careconnect.local/".into());
    let token = DevVerifier::new(&secret, &ns).mint(&format!("dev|{uid}"), "Dev user", &roles, Some(uid), Duration::from_secs(12 * 3600));
    println!("{token}");
}
