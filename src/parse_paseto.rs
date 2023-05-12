use std::io::Error;

use base64::{engine::general_purpose, Engine as _};
use deno_core::error::AnyError;

pub fn parse(token: &str) -> Result<(), AnyError> {
    // let token = "v2.public.Q2dZSXY0bnhvZ1lTQmdqZXBmR2lCaUlMWkdWMlpXeHZjRzFsYm5Rd0FEcXVBUW9rTW1RNVpEWTVOR010WTJJeU1DMDBZalZtTFdFM01URXRaak5tTlRsalltTXpaVFEwRWdOdWFYZ2FESEpsY0d4cGRDMXlaWEJzY3lJTloyOTJZV3d0ZEdWemRHbHVaeW9OVUc5MFpXNTBhV0ZzVTNSNWVFb0hDTGFiOXdZUUFWQUFXZ2x0YjJSbGNtRjBiM0phQ0dWNGNHeHZjbVZ5V2dsa1pYUmxZM1JwZG1WYUVuUmxZV05vWlhKZmRXNTJaWEpwWm1sbFpGb1dkbVZ5YVdacFpXUmZZbTkxYm5SNVgyaDFiblJsY2xJbUNBRVFnSUNBZ0FJWkFBQUFBQUFBNEQ4aEFBQUFBQUFBNEQ4b2dJQ0FnQVF3QVRnQVFBQmdBR29UQ05IcG53RVNERU52WkdWdGIyNXJaWGsxTVhJV1oyaHZjM1IzY21sMFpYSXRaR0YwWVd4dloyZGxjbklSYUhSMGNDMWxaM0psYzNNdGNISnZlSGx5RlhCcFpERXRaMmwwTFdsdUxYUm9aUzF6YUdWc2JISVRjR2xrTVMxdGRXeDBhWEJzWlMxd2IzSjBjM0liY0dsa01TMXpaV05qYjIxd0xYQnZjblF0WkdWMFpXTjBhVzl1Y2hwd2NtVjJaVzUwTFhkaGEyVjFjSE10ZFc1MlpYSnBabWxsWkhJUWNISnZkRzlqYjJ3dGRISmhZMmx1WjNJSGRYTmxMVzVpWkE9PfpuV9LxH0_OPJVSCdSb36QziC0YN_P7jHDXUs8k7cryfQrcASc1CNhHHf3-7kkdESqZVJuPxFG6Bp8zznk4iA0.Q2dad2NtOWtPak1pQ25KbGNHeHBkQzVqYjIwPQ";
    let token_parts = token.split(".").collect::<Vec<_>>();
    if token_parts.len() < 3 {
        return Err(AnyError::new(Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid Token",
        )));
    }

    if token_parts[0] != "v2" || token_parts[1] != "public" {
        return Err(AnyError::new(Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid Token",
        )));
    }

    let decoded = general_purpose::URL_SAFE_NO_PAD
        .decode(token_parts[2].as_bytes())
        .unwrap();
    let decoded_len = decoded.len();
    // currently doesn't verify signature
    let (msg, _sig) = decoded.split_at(decoded_len - 64);
    println!("{}", std::str::from_utf8(msg).unwrap());
    Ok(())
}
