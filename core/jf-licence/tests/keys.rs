//! Licence keys with a key pair made in the test (never a release key): round trips, every
//! error, tolerance of case and separators, revocation and unknown key ids.

use jf_licence::*;

fn pro(id: u64, day: u16) -> Licence {
    Licence {
        id,
        key_id: 0,
        product: Product::Pro,
        issued_day: day,
        flags: 0,
    }
}

#[test]
fn issued_keys_verify_and_round_trip() {
    let (secret, public) = generate_keypair_with_id(7);
    let licence = pro(0x0123_4567_89ab_cdef, 20_409);
    let key = issue(&secret, 7, &licence);
    assert!(key.starts_with("JFP1-"), "{key}");
    assert_eq!(key.len(), 5 + 128 + 15, "16 groups of eight");
    let checked = verify(&key, &[public]).unwrap();
    assert_eq!(checked, Licence { key_id: 7, ..licence });
    assert_eq!(checked.issued_at_ms(), 20_409 * 86_400_000);
    assert_eq!(checked.id_text().len(), 13);
    assert_eq!(checked.id_suffix(), checked.id_text()[9..]);
}

#[test]
fn case_spaces_and_separators_do_not_matter() {
    let (secret, public) = generate_keypair_with_id(1);
    let key = issue(&secret, 1, &pro(42, 20_000));
    let keys = [public];
    let compact = key.replace('-', "");
    let spaced = key.replace('-', " ");
    let lower = key.to_ascii_lowercase();
    let wrapped = format!("  {}\n{}  ", &key[..40], &key[40..]);
    for variant in [compact, spaced, lower, wrapped] {
        assert_eq!(verify(&variant, &keys).unwrap().id, 42, "{variant}");
    }
    // Crockford base32 reads O as 0 and I or L as 1.
    let body = key.trim_start_matches("JFP1-");
    if body.contains('0') {
        let mistyped = format!("JFP1-{}", body.replace('0', "O"));
        assert_eq!(verify(&mistyped, &keys).unwrap().id, 42);
    }
}

#[test]
fn every_error() {
    let (secret, public) = generate_keypair_with_id(3);
    let (_, other) = generate_keypair_with_id(3);
    let key = issue(&secret, 3, &pro(99, 20_100));
    assert_eq!(verify("", &[public]), Err(LicenceError::Malformed));
    assert_eq!(verify("JFP2-ABCD", &[public]), Err(LicenceError::Malformed));
    assert_eq!(
        verify(&key[..key.len() - 3], &[public]),
        Err(LicenceError::Malformed)
    );
    assert_eq!(
        verify(&format!("{key}AB"), &[public]),
        Err(LicenceError::Malformed)
    );
    assert_eq!(
        verify(&key.replace('-', "U"), &[public]),
        Err(LicenceError::Malformed)
    );
    // Unknown key ids, and a key that is not the one that signed it.
    assert_eq!(verify(&key, &[]), Err(LicenceError::UnknownKey));
    assert_eq!(
        verify(&key, &[PublicKey { id: 4, ..public }]),
        Err(LicenceError::UnknownKey)
    );
    assert_eq!(verify(&key, &[other]), Err(LicenceError::BadSignature));
    // One changed character breaks the signature (or the payload's format).
    let mut chars: Vec<char> = key.chars().collect();
    let at = 30;
    chars[at] = if chars[at] == 'A' { 'B' } else { 'A' };
    let tampered: String = chars.into_iter().collect();
    assert!(matches!(
        verify(&tampered, &[public]),
        Err(LicenceError::BadSignature | LicenceError::Malformed)
    ));
    // Revoked ids.
    assert_eq!(verify_with(&key, &[public], &[99]), Err(LicenceError::Revoked));
    assert!(verify_with(&key, &[public], &[98]).is_ok());
}

#[test]
fn keys_for_another_product_are_refused_after_the_signature() {
    // A key with product byte 2, signed properly: only a signed payload can say "another product".
    use ed25519_dalek::Signer;
    let (secret, public) = generate_keypair_with_id(5);
    let mut payload = [0u8; PAYLOAD_BYTES];
    payload[0] = FORMAT;
    payload[1] = 5;
    payload[2] = 2;
    let mut message = CONTEXT.to_vec();
    message.extend_from_slice(&payload);
    let signature = ed25519_dalek::SigningKey::from_bytes(&secret).sign(&message);
    let mut bytes = payload.to_vec();
    bytes.extend_from_slice(&signature.to_bytes());
    let key = format_key(&bytes);
    assert_eq!(verify(&key, &[public]), Err(LicenceError::WrongProduct));
    // Reserved bytes must be zero, signed or not.
    payload[2] = 1;
    payload[15] = 1;
    let mut message = CONTEXT.to_vec();
    message.extend_from_slice(&payload);
    let signature = ed25519_dalek::SigningKey::from_bytes(&secret).sign(&message);
    let mut bytes = payload.to_vec();
    bytes.extend_from_slice(&signature.to_bytes());
    assert_eq!(
        verify(&format_key(&bytes), &[public]),
        Err(LicenceError::Malformed)
    );
}

#[test]
fn test_keys_never_verify_against_the_release_keys() {
    let (secret, _) = generate_keypair_with_id(0);
    let key = issue(&secret, 0, &pro(1, 20_000));
    assert!(verify(&key, &release_keys()).is_err());
}

#[test]
fn public_key_text_round_trips() {
    let (_, public) = generate_keypair_with_id(200);
    let text = public_key_text(&public);
    assert!(text.starts_with("200:"));
    assert_eq!(parse_public_keys(&text).unwrap(), vec![public]);
}
