// Copyright 2024 Tree xie.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::{Error, ProxyPlugin, Result};
use crate::config::{PluginCategory, PluginStep};
use crate::http_extra::HttpResponse;
use crate::http_extra::HTTP_HEADER_NAME_X_REQUEST_ID;
use crate::state::State;
use async_trait::async_trait;
use log::debug;
use nanoid::nanoid;
use pingora::proxy::Session;
use uuid::Uuid;

pub struct RequestId {
    proxy_step: PluginStep,
    algorithm: String,
    size: usize,
}

impl RequestId {
    pub fn new(value: &str, proxy_step: PluginStep) -> Result<Self> {
        debug!("new request id proxy plugin, {value}, {proxy_step:?}");
        if ![PluginStep::Request, PluginStep::ProxyUpstream].contains(&proxy_step) {
            return Err(Error::Invalid {
                category: PluginCategory::RequestId.to_string(),
                message: "Request id should be executed at request or proxy upstream step"
                    .to_string(),
            });
        }
        let arr: Vec<&str> = value.split(' ').collect();
        let algorithm = arr[0].trim().to_string();
        let mut size = 8;
        if arr.len() >= 2 {
            let v = arr[1].parse::<usize>().unwrap();
            if v > 0 {
                size = v;
            }
        }
        Ok(Self {
            size,
            proxy_step,
            algorithm,
        })
    }
}

#[async_trait]
impl ProxyPlugin for RequestId {
    #[inline]
    fn step(&self) -> PluginStep {
        self.proxy_step
    }
    #[inline]
    fn category(&self) -> PluginCategory {
        PluginCategory::RequestId
    }
    #[inline]
    async fn handle(
        &self,
        session: &mut Session,
        ctx: &mut State,
    ) -> pingora::Result<Option<HttpResponse>> {
        let key = HTTP_HEADER_NAME_X_REQUEST_ID.clone();
        if let Some(id) = session.get_header(&key) {
            ctx.request_id = Some(id.to_str().unwrap_or_default().to_string());
            return Ok(None);
        }
        let id = match self.algorithm.as_str() {
            "nanoid" => {
                let size = self.size;
                nanoid!(size)
            }
            _ => Uuid::new_v4().to_string(),
        };
        ctx.request_id = Some(id.clone());
        let _ = session.req_header_mut().insert_header(key, &id);
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::RequestId;
    use crate::state::State;
    use crate::{config::PluginStep, plugin::ProxyPlugin};
    use pingora::proxy::Session;
    use pretty_assertions::assert_eq;
    use tokio_test::io::Builder;

    #[tokio::test]
    async fn test_request_id() {
        let id = RequestId::new("nanoid 10", PluginStep::Request).unwrap();

        let headers = ["X-Request-Id: 123"].join("\r\n");
        let input_header = format!("GET /vicanso/pingap?size=1 HTTP/1.1\r\n{headers}\r\n\r\n");
        let mock_io = Builder::new().read(input_header.as_bytes()).build();
        let mut session = Session::new_h1(Box::new(mock_io));
        session.read_request().await.unwrap();

        let mut state = State::default();
        let result = id.handle(&mut session, &mut state).await.unwrap();
        assert_eq!(true, result.is_none());
        assert_eq!("123", state.request_id.unwrap_or_default());

        let headers = ["Accept-Encoding: gzip"].join("\r\n");
        let input_header = format!("GET /vicanso/pingap?size=1 HTTP/1.1\r\n{headers}\r\n\r\n");
        let mock_io = Builder::new().read(input_header.as_bytes()).build();
        let mut session = Session::new_h1(Box::new(mock_io));
        session.read_request().await.unwrap();

        let mut state = State::default();
        let result = id.handle(&mut session, &mut state).await.unwrap();
        assert_eq!(true, result.is_none());
        assert_eq!(10, state.request_id.unwrap_or_default().len());
    }
}
