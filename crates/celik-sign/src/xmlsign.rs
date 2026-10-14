use crate::Result;

/// Anything that can produce an RSA-SHA256 signature over a SHA-256 digest.
pub trait DigestSigner {
    fn sign_sha256_digest(&self, digest: &[u8]) -> Result<Vec<u8>>;
}

/// Produce an enveloped XML-DSig signature (same output as bas-celik `SignXMLBytes`).
pub fn sign_xml(_xml: &[u8], _signer: &dyn DigestSigner, _cert_der: &[u8]) -> Result<String> { todo!() }
