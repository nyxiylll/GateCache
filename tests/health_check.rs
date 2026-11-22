use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;
use Gatecache::app;

#[tokio::test]
async fn health_check_returns_200() {
    let app = app();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health_check")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK)
}
