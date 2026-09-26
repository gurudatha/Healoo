//! Local certificate authority for LAN HTTPS.
//!
//! On first start we create `healoo-local-ca.crt` (install it on test devices once) and a server
//! certificate signed by it, valid for this machine's LAN IP, localhost and the Android emulator
//! host alias 10.0.2.2. When the LAN IP changes (different Wi-Fi) only the server certificate is
//! re-issued, so devices keep trusting the same CA.

use anyhow::{Context, Result};
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, KeyPair, KeyUsagePurpose, SanType};
use std::{
    fs,
    net::{IpAddr, Ipv4Addr},
    path::{Path, PathBuf},
};

pub fn ensure_local_certs(dir: &Path, lan_ip: IpAddr) -> Result<(PathBuf, PathBuf, PathBuf)> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let ca_crt = dir.join("healoo-local-ca.crt");
    let ca_key = dir.join("healoo-local-ca.key");
    let tag = lan_ip.to_string().replace([':', '.'], "-");
    let srv_crt = dir.join(format!("server-{tag}.crt"));
    let srv_key = dir.join(format!("server-{tag}.key"));

    if !(ca_crt.exists() && ca_key.exists()) {
        let key = KeyPair::generate()?;
        let mut p = CertificateParams::new(Vec::<String>::new())?;
        p.distinguished_name.push(DnType::CommonName, "Healoo Local Trial CA");
        p.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        p.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        let cert = p.self_signed(&key)?;
        fs::write(&ca_crt, cert.pem())?;
        fs::write(&ca_key, key.serialize_pem())?;
        tracing::info!("created local CA at {}", ca_crt.display());
    }

    if !(srv_crt.exists() && srv_key.exists()) {
        let ca_key_pair = KeyPair::from_pem(&fs::read_to_string(&ca_key)?)?;
        let ca_params = CertificateParams::from_ca_cert_pem(&fs::read_to_string(&ca_crt)?)?;
        let ca_cert = ca_params.self_signed(&ca_key_pair)?;

        let key = KeyPair::generate()?;
        let mut p = CertificateParams::new(vec!["localhost".to_string(), "healoo-trial.local".to_string()])?;
        p.distinguished_name.push(DnType::CommonName, "Healoo trial server");
        for ip in [lan_ip, IpAddr::V4(Ipv4Addr::LOCALHOST), IpAddr::V4(Ipv4Addr::new(10, 0, 2, 2))] {
            p.subject_alt_names.push(SanType::IpAddress(ip));
        }
        let cert = p.signed_by(&key, &ca_cert, &ca_key_pair)?;
        fs::write(&srv_crt, cert.pem())?;
        fs::write(&srv_key, key.serialize_pem())?;
        tracing::info!("issued server certificate for {lan_ip}");
    }
    Ok((srv_crt, srv_key, ca_crt))
}
