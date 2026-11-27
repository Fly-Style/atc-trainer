use std::time::Duration;

use atc_shared::{
    ids::SessionHash,
    protocol::{ApiError, JoinStudentRequest, JoinStudentResponse},
    sector::SectorIdentity,
};

use crate::core::http::{HttpClient, HttpError};

pub(super) fn host_address(address: &str) -> Result<String, &'static str> {
    let address = address.trim();
    let url = if address.contains("://") {
        reqwest::Url::parse(address)
    } else {
        reqwest::Url::parse(&format!("http://{address}"))
    };
    let error = "Enter a host address, for example http://127.0.0.1:8080.";
    let url = url.map_err(|_| error)?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.has_host()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(error);
    }
    Ok(url.origin().ascii_serialization())
}

pub(super) async fn join(
    address: String,
    invitation: String,
) -> Result<JoinStudentResponse, String> {
    let client = HttpClient::new(address);
    let request = JoinStudentRequest {
        session_hash: SessionHash::new(invitation),
    };
    tokio::time::timeout(Duration::from_secs(15), client.join_student(&request))
        .await
        .map_err(|_| "The host did not respond in time.".to_owned())?
        .map_err(|error| match error {
            HttpError::Status { status, body } => {
                match serde_json::from_str::<ApiError>(&body)
                    .ok()
                    .as_ref()
                    .map(|body| body.error.code.as_str())
                {
                    Some("invalid_session_hash") => "Invalid student invitation.".into(),
                    Some("student_position_occupied") => {
                        "The student position is already occupied.".into()
                    }
                    Some("student_position_missing") => "The host has no student position.".into(),
                    _ => format!("The host rejected the join request (HTTP {status})."),
                }
            }
            HttpError::Transport(_) => {
                "Could not reach the host. Check the address and try again.".into()
            }
            HttpError::Parse(_) => "The host returned an incompatible join response.".into(),
        })
}

pub(super) fn matches_sector(
    admission: &JoinStudentResponse,
    local: Option<&SectorIdentity>,
) -> bool {
    local.is_some_and(|identity| identity.sector_id == admission.sector_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host;
    use atc_shared::ids::SectorId;

    #[test]
    fn join_address_accepts_a_bare_host_or_copied_url() {
        for address in ["127.0.0.1:8080", " http://127.0.0.1:8080/ "] {
            assert_eq!(host_address(address), Ok("http://127.0.0.1:8080".into()));
        }
    }

    #[test]
    fn join_address_rejects_non_http_urls_and_paths() {
        for address in [
            "",
            "ftp://example.test",
            "http://example.test/other",
            "http://user@example.test",
        ] {
            assert!(host_address(address).is_err(), "{address}");
        }
    }

    #[tokio::test]
    async fn an_admitted_student_can_correct_the_local_sector_without_rejoining() {
        let expected = SectorIdentity {
            name: "Test sector".into(),
            sector_id: SectorId::new("aurora-expected"),
        };
        let wrong = SectorIdentity {
            name: expected.name.clone(),
            sector_id: SectorId::new("aurora-other"),
        };
        let host = host::prepare("127.0.0.1:0", &expected).unwrap();
        let listener = tokio::net::TcpListener::from_std(host.listener).unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, host.router).await });
        let address = host.info.address;
        let invitation = host.info.join_string;

        assert_eq!(
            join(address.clone(), "wrong-invitation".into())
                .await
                .unwrap_err(),
            "Invalid student invitation."
        );
        let admission = join(address.clone(), invitation.clone()).await.unwrap();
        assert!(!matches_sector(&admission, None));
        assert!(!matches_sector(&admission, Some(&wrong)));
        assert!(matches_sector(&admission, Some(&expected)));
        assert_eq!(
            join(address, invitation).await.unwrap_err(),
            "The student position is already occupied."
        );

        server.abort();
    }
}
