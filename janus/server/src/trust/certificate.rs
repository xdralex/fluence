/*
 * Copyright 2019 Fluence Labs Limited
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use crate::trust::key_pair::KeyPair;
use crate::trust::trust::{Trust, TRUST_LEN};
use libp2p::identity::ed25519::PublicKey;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Serialization format of a certificate.
/// TODO
const FORMAT: &[u8; 2] = &[0, 0];
/// Serialization format version of a certificate.
/// TODO
const VERSION: &[u8; 4] = &[0, 0, 0, 0];

/// Chain of trust from root certificate to tail certificate.
#[derive(Debug, Clone, PartialEq)]
pub struct Certificate {
    chain: Vec<Trust>,
}

impl Certificate {
    /// Creates new certificate with root trust (self-signed public key) from a key pair.
    #[allow(dead_code)]
    pub fn issue_root(kp: &KeyPair, for_pk: PublicKey, expire: Duration) -> Certificate {
        let root_expiration = Duration::from_millis(u64::max_value());

        let root_sig = Certificate::sign(kp, &kp.key_pair.public(), root_expiration);

        let root_trust = Trust {
            pk: kp.key_pair.public(),
            expires_at: root_expiration,
            signature: root_sig,
        };

        let sig = Certificate::sign(kp, &for_pk, expire);
        let trust = Trust {
            pk: for_pk,
            expires_at: expire,
            signature: sig,
        };

        let chain = vec![root_trust, trust];
        Self { chain }
    }

    /// Adds a new trust into chain of trust in certificate.
    #[allow(dead_code)]
    pub fn issue(
        kp: &KeyPair,
        for_pk: PublicKey,
        own_cert: &Certificate,
        expire: Duration,
    ) -> Result<Certificate, String> {
        let cur_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        // first, verify given certificate
        Certificate::verify(own_cert, &[own_cert.chain[0].pk.clone()], cur_time)?;

        let pk = kp.key_pair.public();

        // find if the public key is exists in a chain
        let mut previous_trust_num: i32 = -1;
        for pk_id in 0..own_cert.chain.len() {
            if own_cert.chain[pk_id].pk == pk {
                previous_trust_num = pk_id as i32;
            }
        }

        if previous_trust_num == -1 {
            return Err("Your public key should be in certificate.".to_string());
        };

        // splitting old chain to add new trust after given public key
        let mut new_chain = own_cert
            .chain
            .split_at((previous_trust_num + 1) as usize)
            .0
            .to_vec();

        let sig = Certificate::sign(kp, &for_pk, expire);
        let trust = Trust {
            pk: for_pk,
            expires_at: expire,
            signature: sig,
        };

        new_chain.push(trust);

        Ok(Self { chain: new_chain })
    }

    /// Verifies that a certificate is valid and you trust to this certificate.
    #[allow(dead_code)]
    pub fn verify(
        cert: &Certificate,
        trusted_roots: &[PublicKey],
        time: Duration,
    ) -> Result<(), String> {
        let chain = &cert.chain;

        if chain.len() < 2 {
            return Err("The certificate must have at least 2 trusts".to_string());
        }

        // check if every element in a chain is not expired and has the correct signature
        for trust_id in (1..chain.len()).rev() {
            let trust = &chain[trust_id];

            if trust.expires_at < time {
                return Err(format!("{} certificate in chain is expired.", trust_id));
            }

            let trust_giver = &chain[trust_id - 1];

            let msg = Certificate::pk_and_expiration_to_bytes(&trust.pk, trust.expires_at);

            let verify_result = KeyPair::verify(&trust_giver.pk, &msg, trust.signature.as_slice());
            if !verify_result {
                return Err(format!("{} trust in chain is forged.", trust_id));
            }
        }

        let root = &chain[0];

        let msg = Certificate::pk_and_expiration_to_bytes(&root.pk, root.expires_at);

        if !KeyPair::verify(&root.pk, &msg, root.signature.as_slice()) {
            return Err("Root trust in chain is forged.".to_string());
        }
        if !trusted_roots.contains(&root.pk) {
            return Err("Certificate does not contain a trusted root.".to_string());
        }

        Ok(())
    }

    /// Convert certificate to byte format
    /// 2 format + 4 version + (64 signature + 32 public key + 8 expiration) * number of trusts
    #[allow(dead_code)]
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded =
            Vec::with_capacity(FORMAT.len() + VERSION.len() + TRUST_LEN * self.chain.len());
        encoded.extend_from_slice(FORMAT);
        encoded.extend_from_slice(VERSION);

        for t in &self.chain {
            encoded.extend(t.encode());
        }

        encoded
    }

    #[allow(dead_code)]
    pub fn decode(arr: &[u8]) -> Result<Certificate, String> {
        let trusts_offset = arr.len() - 2 - 4;
        if trusts_offset % TRUST_LEN != 0 {
            return Err("Incorrect length of an array. Should be 2 bytes of a format, 4 bytes of a version and 104 bytes for each trust. ".to_string());
        }

        let number_of_trusts = trusts_offset / TRUST_LEN;

        if number_of_trusts < 2 {
            return Err("The certificate must have at least 2 trusts.".to_string());
        }

        let _format = &arr[0..1];
        let _version = &arr[2..5];

        let mut chain = Vec::with_capacity(number_of_trusts);

        for i in 0..number_of_trusts {
            let from = i * TRUST_LEN + 6;
            let to = (i + 1) * TRUST_LEN + 6;
            let slice = &arr[from..to];
            let t = Trust::decode(slice)?;
            chain.push(t);
        }

        Ok(Self { chain })
    }

    fn pk_and_expiration_to_bytes(pk: &PublicKey, expire: Duration) -> [u8; 40] {
        let pk_encoded = pk.encode();
        let expire_encoded: [u8; 8] = (expire.as_millis() as u64).to_le_bytes();
        let mut msg = [0; 40];

        msg[..32].clone_from_slice(&pk_encoded[..32]);
        msg[33..40].clone_from_slice(&expire_encoded[0..7]);

        msg
    }

    fn sign(kp: &KeyPair, pk: &PublicKey, expire: Duration) -> Vec<u8> {
        let msg = Certificate::pk_and_expiration_to_bytes(pk, expire);

        kp.sign(&msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_second() -> Duration {
        Duration::new(1, 0)
    }

    fn cur_time() -> Duration {
        Duration::from_millis(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        )
    }

    #[test]
    fn test_serialization_deserialization() {
        let (_root_kp, second_kp, cert) = generate_root_cert();

        let cur_time = cur_time();

        let third_kp = KeyPair::generate();

        let new_cert = Certificate::issue(
            &second_kp,
            third_kp.key_pair.public(),
            &cert,
            cur_time.checked_add(one_second()).unwrap(),
        )
        .unwrap();

        let serialized = new_cert.encode();
        let deserialized = Certificate::decode(serialized.as_slice());
        println!("{:?}", deserialized);

        assert!(deserialized.is_ok());
        let after_cert = deserialized.unwrap();
        assert_eq!(&new_cert.chain[0], &after_cert.chain[0]);
        assert_eq!(&new_cert, &after_cert);
    }

    #[test]
    fn test_small_chain() {
        let bad_cert = Certificate { chain: Vec::new() };

        let check = Certificate::verify(&bad_cert, &[], cur_time());
        assert!(check.is_err());
    }

    fn generate_root_cert() -> (KeyPair, KeyPair, Certificate) {
        let root_kp = KeyPair::generate();
        let second_kp = KeyPair::generate();

        let cur_time = cur_time();

        (
            root_kp.clone(),
            second_kp.clone(),
            Certificate::issue_root(
                &root_kp,
                second_kp.show_public_key(),
                cur_time.checked_add(one_second()).unwrap(),
            ),
        )
    }

    #[test]
    fn test_issue_cert() {
        let (root_kp, second_kp, cert) = generate_root_cert();
        let trusted_roots = [root_kp.show_public_key()];

        // we don't need nanos for serialization, etc
        let cur_time = Duration::from_millis(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        );

        let third_kp = KeyPair::generate();

        let new_cert = Certificate::issue(
            &second_kp,
            third_kp.key_pair.public(),
            &cert,
            cur_time.checked_add(one_second()).unwrap(),
        );
        assert_eq!(new_cert.is_ok(), true);
        let new_cert = new_cert.unwrap();

        assert_eq!(new_cert.chain.len(), 3);
        assert_eq!(new_cert.chain[0].pk, root_kp.show_public_key());
        assert_eq!(new_cert.chain[1].pk, second_kp.show_public_key());
        assert_eq!(new_cert.chain[2].pk, third_kp.show_public_key());
        assert!(Certificate::verify(&new_cert, &trusted_roots, cur_time).is_ok());
    }

    #[test]
    fn text_cert_expiration() {
        let (root_kp, second_kp, cert) = generate_root_cert();
        let trusted_roots = [root_kp.show_public_key()];
        let cur_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        let third_kp = KeyPair::generate();

        let new_cert = Certificate::issue(
            &second_kp,
            third_kp.key_pair.public(),
            &cert,
            cur_time.checked_sub(one_second()).unwrap(),
        )
        .unwrap();

        assert!(Certificate::verify(&new_cert, &trusted_roots, cur_time).is_err());
    }

    #[test]
    fn test_issue_end_chain() {
        let (root_kp, second_kp, cert) = generate_root_cert();
        let trusted_roots = [root_kp.show_public_key()];
        let cur_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        let third_kp = KeyPair::generate();
        let fourth_kp = KeyPair::generate();

        let new_cert = Certificate::issue(
            &second_kp,
            third_kp.key_pair.public(),
            &cert,
            cur_time.checked_add(one_second()).unwrap(),
        )
        .unwrap();
        let new_cert = Certificate::issue(
            &third_kp,
            fourth_kp.key_pair.public(),
            &new_cert,
            cur_time.checked_add(one_second()).unwrap(),
        );

        assert_eq!(new_cert.is_ok(), true);
        let new_cert = new_cert.unwrap();

        assert_eq!(new_cert.chain.len(), 4);
        assert_eq!(new_cert.chain[0].pk, root_kp.show_public_key());
        assert_eq!(new_cert.chain[1].pk, second_kp.show_public_key());
        assert_eq!(new_cert.chain[2].pk, third_kp.show_public_key());
        assert_eq!(new_cert.chain[3].pk, fourth_kp.show_public_key());
        assert!(Certificate::verify(&new_cert, &trusted_roots, cur_time).is_ok());
    }

    #[test]
    fn test_issue_middle_chain() {
        let (root_kp, second_kp, cert) = generate_root_cert();
        let trusted_roots = [root_kp.show_public_key()];
        let cur_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        let third_kp = KeyPair::generate();
        let fourth_kp = KeyPair::generate();

        let new_cert = Certificate::issue(
            &second_kp,
            third_kp.key_pair.public(),
            &cert,
            cur_time.checked_add(one_second()).unwrap(),
        )
        .unwrap();
        let new_cert = Certificate::issue(
            &second_kp,
            fourth_kp.key_pair.public(),
            &new_cert,
            cur_time.checked_add(one_second()).unwrap(),
        );

        assert_eq!(new_cert.is_ok(), true);
        let new_cert = new_cert.unwrap();

        assert_eq!(new_cert.chain.len(), 3);
        assert_eq!(new_cert.chain[0].pk, root_kp.show_public_key());
        assert_eq!(new_cert.chain[1].pk, second_kp.show_public_key());
        assert_eq!(new_cert.chain[2].pk, fourth_kp.show_public_key());
        assert!(Certificate::verify(&new_cert, &trusted_roots, cur_time).is_ok());
    }

    #[test]
    fn test_no_cert_in_chain() {
        let (_root_kp, _second_kp, cert) = generate_root_cert();
        let cur_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        let bad_kp = KeyPair::generate();
        let new_cert_bad = Certificate::issue(
            &bad_kp,
            bad_kp.key_pair.public(),
            &cert,
            cur_time.checked_add(one_second()).unwrap(),
        );
        assert_eq!(new_cert_bad.is_err(), true);
    }

    #[test]
    fn test_no_trusted_root_in_chain() {
        let (_root_kp, second_kp, cert) = generate_root_cert();
        let cur_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        let trusted_roots = [second_kp.show_public_key()];
        assert!(Certificate::verify(&cert, &trusted_roots, cur_time).is_err());
        assert!(Certificate::verify(&cert, &[], cur_time).is_err());
    }

    #[test]
    fn test_forged_cert() {
        let (root_kp, _second_kp, cert) = generate_root_cert();
        let cur_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let trusted_roots = [root_kp.show_public_key()];

        // forged cert
        let mut bad_chain = cert.chain.clone();
        bad_chain.remove(0);
        let bad_cert = Certificate { chain: bad_chain };

        assert!(Certificate::verify(&bad_cert, &trusted_roots, cur_time).is_err());
    }

    #[test]
    fn test_generate_root_cert() {
        let (root_kp, second_kp, cert) = generate_root_cert();
        let cur_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        let trusted_roots = [root_kp.show_public_key()];

        assert_eq!(cert.chain.len(), 2);
        assert_eq!(cert.chain[0].pk, root_kp.show_public_key());
        assert_eq!(cert.chain[1].pk, second_kp.show_public_key());
        assert!(Certificate::verify(&cert, &trusted_roots, cur_time).is_ok());
    }
}
