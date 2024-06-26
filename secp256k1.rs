extern crate rand;
extern crate secp256k1;
//use rand::rngs::OsRng;
use secp256k1::bitcoin_hashes::sha256;
use secp256k1::rand::rngs::OsRng;
use secp256k1::{All, Message, PublicKey, Secp256k1, SecretKey, Signature};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use secp256k1::rand::thread_rng;


/*
rand = "0.8.5"
bitcoin_hashes = "0.10.0"
hex-literal = "0.3.3"

[dependencies.secp256k1]
features = ["rand", "bitcoin_hashes","rand-std"]
version = "0.20.0"
*/
pub struct KeyMaster {
    pub secp: Secp256k1<All>,
    pub public_key: String,
    pub secret_key: String,
}

/* Keymaster holds the keys for the transactions */
impl KeyMaster {
    pub fn new() -> KeyMaster {
        let secp = Secp256k1::new();
        let mut rng = OsRng::new().expect("OsRng");
        let secret_key = SecretKey::new(&mut rng);
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        return KeyMaster {
            secp: secp,
            secret_key: secret_key.to_string(),
            public_key: public_key.to_string(),
        };
    }

    /* To start it from already generated values */
    pub fn holding_these(secret_key: &str, public_key: &str) -> KeyMaster {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_str(secret_key).unwrap();
        let public_key = PublicKey::from_str(public_key).unwrap();
        return KeyMaster {
            secp: secp,
            secret_key: secret_key.to_string(),
            public_key: public_key.to_string(),
        };
    }

    /* Sign a message */
    pub fn sign(&self, message: String) -> String {
        let message_ = Message::from_hashed_data::<sha256::Hash>(message.as_bytes());
        return self
            .secp
            .sign(
                &message_,
                &SecretKey::from_str(&self.secret_key[..]).unwrap(),
            )
            .to_string();
    }

    /* Verify a message */
    pub fn verify(&self, message: String, signature: String) -> bool {
        let message_ = Message::from_hashed_data::<sha256::Hash>(message.as_bytes());
        return self
            .secp
            .verify(
                &message_,
                &Signature::from_str(&signature[..]).unwrap(),
                &PublicKey::from_str(&self.public_key[..]).unwrap(),
            )
            .is_ok();
    }

    /* Verify a message using another public key */
    pub fn verify_with_public_key(
        &self,
        public_key: String,
        message: String,
        signature: String,
    ) -> bool {
        let message_ = Message::from_hashed_data::<sha256::Hash>(message.as_bytes());
        return self
            .secp
            .verify(
                &message_,
                &Signature::from_str(&signature[..]).unwrap(),
                &PublicKey::from_str(&public_key[..]).unwrap(),
            )
            .is_ok();
    }
}

pub fn generate_key_pair() -> (String, String) {
    // Create a Secp256k1 context
    let secp = Secp256k1::new();

    // Generate a random secret key
    let mut rng = thread_rng();
    let secret_key = SecretKey::new(&mut rng);

    // Derive the corresponding public key
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    // Convert keys to hexadecimal strings
    let secret_key_hex = format!("{:x}", secret_key);
    let public_key_hex = format!("{:x}", public_key);

    (secret_key_hex, public_key_hex)
}


/* sha256 */
pub fn hash_string(in_str: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(in_str);
    return format!("{:x}", hasher.finalize());
}

fn main() {
    let mut k = KeyMaster::new();
    let orig = k.secret_key;
    let corig = k.public_key;

    println!("Original public key: {corig}, private key: {orig}");

    let (publ,priva) = generate_key_pair();
    println!("{priva} - private key, {publ} - public key");

}
