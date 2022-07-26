// SPDX-FileCopyrightText: 2022 Profian Inc. <opensource@profian.com>
// SPDX-License-Identifier: AGPL-3.0-only

use super::{repos, tags, trees, users};

use drawbridge_type::{RepositoryName, TagName, TreePath, UserName};

use axum::body::Body;
use axum::handler::Handler;
use axum::http::{Method, Request, StatusCode};
use axum::response::IntoResponse;
use log::trace;
use tower::Service;

/// Parses the URI of `req` and routes it to respective component.
pub async fn handle(mut req: Request<Body>) -> impl IntoResponse {
    trace!(target: "app::handle", "begin HTTP request handling {:?}", req);
    let path = req.uri().path().trim_start_matches('/');
    let path = path
        .strip_prefix("api")
        .ok_or((StatusCode::NOT_FOUND, format!("Route `/{path}` not found")))?
        .trim_start_matches('/')
        // TODO: Parse SemVer, support v0, v0.1 etc.
        .strip_prefix("v0.1.0")
        .ok_or((
            StatusCode::NOT_IMPLEMENTED,
            "Unsupported API version".into(),
        ))?
        .trim_start_matches('/');
    let (head, tail) = path
        .split_once("/_")
        .map(|(left, right)| (left.trim_end_matches('/').to_string(), format!("_{right}")))
        .unwrap_or((path.trim_end_matches('/').to_string(), "".into()));
    if head.is_empty() {
        return Err((StatusCode::NOT_FOUND, format!("Route `/{path}` not found")));
    }

    let extensions = req.extensions_mut();

    let (user, head) = head.split_once('/').unwrap_or((&head.trim_start_matches('/'), ""));
    let user = user.trim_end_matches('/').parse::<UserName>().map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Failed to parse user name: {e}"),
        )
    })?;
    trace!(target: "app::handle", "parsed user name: `{user}`");
    assert_eq!(extensions.insert(user), None, "duplicate user name");
    if head.is_empty() {
        return match *req.method() {
            Method::HEAD => Ok(users::head.into_service().call(req).await.into_response()),
            Method::GET => Ok(users::get.into_service().call(req).await.into_response()),
            Method::PUT => Ok(users::put.into_service().call(req).await.into_response()),
            _ => Err((
                StatusCode::METHOD_NOT_ALLOWED,
                "Method not allowed for user endpoint".into(),
            )),
        };
    }

    let repo = head.trim_end_matches('/').parse::<RepositoryName>().map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Failed to parse repository name: {e}"),
        )
    })?;
    trace!(target: "app::handle", "parsed repository name: `{repo}`");
    assert_eq!(extensions.insert(repo), None, "duplicate repository name");

    //let mut tail = tail.splitn(4, '/');
    let mut tail = tail.split('/').filter(|x| !x.is_empty());

    match (tail.next(), tail.next(), tail.next()) {
        (None | Some(""), None, None) => match *req.method() {
            Method::HEAD => Ok(repos::head.into_service().call(req).await.into_response()),
            Method::GET => Ok(repos::get.into_service().call(req).await.into_response()),
            Method::PUT => Ok(repos::put.into_service().call(req).await.into_response()),
            _ => Err((
                StatusCode::METHOD_NOT_ALLOWED,
                "Method not allowed for repository endpoint".into(),
            )),
        },
        (Some("_tag"), None, None) => match *req.method() {
            Method::GET => Ok(tags::query.into_service().call(req).await.into_response()),
            _ => Err((
                StatusCode::METHOD_NOT_ALLOWED,
                "Method not allowed for repository tag query endpoint".into(),
            )),
        },
        (Some("_tag"), Some(tag), prop @ (None | Some("tree"))) => {
            let tag = tag
                .trim_start_matches('/')
                .trim_end_matches('/')
                .parse::<TagName>()
                .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Failed to parse tag name: {e}"),
                )
            })?;
            trace!(target: "app::handle", "parsed tag name: `{tag}`");
            assert_eq!(extensions.insert(tag), None, "duplicate tag name");

            if prop.is_none() {
                return match *req.method() {
                    Method::HEAD => Ok(tags::head.into_service().call(req).await.into_response()),
                    Method::GET => Ok(tags::get.into_service().call(req).await.into_response()),
                    Method::PUT => Ok(tags::put.into_service().call(req).await.into_response()),
                    _ => Err((
                        StatusCode::METHOD_NOT_ALLOWED,
                        "Method not allowed for tag endpoint".into(),
                    )),
                };
            }

            let path = tail
                .next()
                .unwrap_or("")
                .trim_start_matches('/')
                .trim_end_matches('/')
                .parse::<TreePath>()
                .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Failed to parse tree path: {e}"),
                )
            })?;
            trace!(target: "app::handle", "parsed tree path: `{path}`");
            assert_eq!(extensions.insert(path), None, "duplicate tree path");
            match *req.method() {
                Method::HEAD => Ok(trees::head.into_service().call(req).await.into_response()),
                Method::GET => Ok(trees::get.into_service().call(req).await.into_response()),
                Method::PUT => Ok(trees::put.into_service().call(req).await.into_response()),
                _ => Err((
                    StatusCode::METHOD_NOT_ALLOWED,
                    "Method not allowed for tag tree endpoint".into(),
                )),
            }
        }
        _ => Err((
            StatusCode::NOT_FOUND,
            "Route not found on repository".into(),
        )),
    }
}
