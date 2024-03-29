use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use homeval_services::ClientInfo;
use pasetors::{token::UntrustedToken, version2::V2, Public};
use prost::Message;
use std::io::Error;

#[cfg(feature = "verify_connections")]
static KEYS: tokio::sync::OnceCell<std::collections::HashMap<String, String>> =
    tokio::sync::OnceCell::const_new();

#[cfg(feature = "verify_connections")]
use tracing::warn;

fn parse_noverify(input: &str) -> Result<(Vec<u8>, bool)> {
    let token: UntrustedToken<Public, V2> = match UntrustedToken::try_from(input) {
        Ok(token) => token,
        Err(_err) => {
            return Err(Error::new(
                std::io::ErrorKind::InvalidData,
                "Parsing error on paseto token",
            )
            .into())
        }
    };
    // let token_parts = token.split('.').collect::<Vec<_>>();
    // if token_parts.len() < 3 {
    //     return Err(Error::new(std::io::ErrorKind::InvalidData, "Invalid Token").into());
    // }

    // if token_parts[0] != "v2" || token_parts[1] != "public" {
    //     return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid Token").into());
    // }

    // let decoded = general_purpose::URL_SAFE_NO_PAD.decode(token_parts[2].as_bytes())?;
    // let decoded_len = decoded.len();
    // // currently doesn't verify signature
    // let (msg, _sig) = decoded.split_at(decoded_len - 64);
    Ok((token.untrusted_payload().to_vec(), false))
}

#[cfg(feature = "verify_connections")]
async fn init_keys() -> Result<std::collections::HashMap<String, String>> {
    use http_body_util::{BodyExt, Collected};
    use hyper_tls::HttpsConnector;
    use hyper_util::client::legacy::{connect::HttpConnector, Client};
    use hyper_util::rt::TokioExecutor;

    let key_get = std::env::var("HOMEVAL_PASETO_KEY_URL")?;

    let https = HttpsConnector::new();
    let client: Client<HttpsConnector<HttpConnector>, Collected<_>> =
        Client::builder(TokioExecutor::new())
            .build::<HttpsConnector<HttpConnector>, Collected<prost::bytes::Bytes>>(https);

    let mut _body = client.get(hyper::Uri::try_from(key_get)?).await?;

    let body = _body.body_mut().collect().await?.to_bytes();

    Ok(serde_json::from_slice(&body)?)
}

#[cfg(feature = "verify_connections")]
async fn parse_verify(input: &str) -> Result<(Vec<u8>, bool)> {
    let keys = KEYS.get_or_try_init(init_keys).await?;
    let token = match pasetors::token::UntrustedToken::try_from(input) {
        Ok(token) => token,
        Err(_err) => {
            return Err(Error::new(
                std::io::ErrorKind::InvalidData,
                "Parsing error on paseto token",
            )
            .into())
        }
    };

    let _authority = general_purpose::STANDARD.decode(token.untrusted_footer())?;
    let authority = goval::GovalSigningAuthority::decode(_authority.as_slice())?;

    let key_id;

    match authority.cert {
        Some(cert) => match cert {
            goval::goval_signing_authority::Cert::KeyId(key) => key_id = key,
            goval::goval_signing_authority::Cert::SignedCert(_) => {
                return Err(Error::new(
                    std::io::ErrorKind::InvalidData,
                    "SignedCert is not accepted yet",
                )
                .into())
            }
        },
        None => {
            return Err(Error::new(std::io::ErrorKind::InvalidData, "No cert in paseto").into())
        }
    }

    let pubkey: pasetors::keys::AsymmetricPublicKey<pasetors::version2::V2>;

    if let Some(key) = keys.get(&key_id) {
        pubkey = pasetors::keys::AsymmetricPublicKey::from(
            general_purpose::STANDARD.decode(key)?.as_slice(),
        )
        .unwrap();
    } else {
        return Err(Error::new(
            std::io::ErrorKind::InvalidData,
            "Cert in paseto couldn't be found",
        )
        .into());
    }

    let result = match pasetors::version2::PublicToken::verify(&pubkey, &token, None) {
        Ok(trusted) => trusted,
        Err(err) => {
            return Err(Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Paseto invalid: `{:#?}`", err),
            )
            .into());
        }
    };

    Ok((result.payload().as_bytes().to_vec(), true))
}

pub async fn parse(token: &str) -> Result<ClientInfo> {
    let msg;
    let is_secure;

    #[cfg(not(feature = "verify_connections"))]
    {
        (msg, is_secure) = parse_noverify(token)?;
    }

    #[cfg(feature = "verify_connections")]
    {
        match parse_verify(token).await {
            Ok(res) => {
                (msg, is_secure) = res;
            }
            Err(err) => {
                warn!(
                    %err,
                    "Error in paseto parser + verification, falling back to non verifying parser"
                );
                (msg, is_secure) = parse_noverify(token)?;
            }
        }
    }

    let _inner = general_purpose::STANDARD.decode(msg)?;
    let inner = goval::ReplToken::decode(_inner.as_slice())?;

    match inner.presenced {
        Some(user) => Ok(ClientInfo {
            is_secure,

            username: user.bearer_name,
            id: user.bearer_id,
        }),
        None => Ok(ClientInfo::default()),
    }
}
