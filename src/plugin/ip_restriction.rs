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

use super::{
    get_hash_key, get_step_conf, get_str_conf, get_str_slice_conf, Error,
    Plugin, Result,
};
use crate::config::{PluginCategory, PluginConf, PluginStep};
use crate::http_extra::HttpResponse;
use crate::state::State;
use crate::util;
use async_trait::async_trait;
use bytes::Bytes;
use http::StatusCode;
use pingora::proxy::Session;
use tracing::debug;

pub struct IpRestriction {
    plugin_step: PluginStep,
    ip_rules: util::IpRules,
    restriction_category: String,
    forbidden_resp: HttpResponse,
    hash_value: String,
}

impl TryFrom<&PluginConf> for IpRestriction {
    type Error = Error;
    fn try_from(value: &PluginConf) -> Result<Self> {
        let hash_value = get_hash_key(value);
        let step = get_step_conf(value);

        let ip_rules =
            util::IpRules::new(&get_str_slice_conf(value, "ip_list"));
        let mut message = get_str_conf(value, "message");
        if message.is_empty() {
            message = "Request is forbidden".to_string();
        }
        let params = Self {
            hash_value,
            plugin_step: step,
            ip_rules,
            restriction_category: get_str_conf(value, "type"),
            forbidden_resp: HttpResponse {
                status: StatusCode::FORBIDDEN,
                body: Bytes::from(message),
                ..Default::default()
            },
        };
        if PluginStep::Request != params.plugin_step {
            return Err(Error::Invalid {
                category: PluginCategory::IpRestriction.to_string(),
                message: "Ip restriction plugin should be executed at request or proxy upstream step".to_string(),
            });
        }
        Ok(params)
    }
}

impl IpRestriction {
    pub fn new(params: &PluginConf) -> Result<Self> {
        debug!(params = params.to_string(), "new ip restriction plugin");
        Self::try_from(params)
    }
}

#[async_trait]
impl Plugin for IpRestriction {
    #[inline]
    fn hash_key(&self) -> String {
        self.hash_value.clone()
    }
    #[inline]
    async fn handle_request(
        &self,
        step: PluginStep,
        session: &mut Session,
        ctx: &mut State,
    ) -> pingora::Result<Option<HttpResponse>> {
        if step != self.plugin_step {
            return Ok(None);
        }
        let ip = if let Some(ip) = &ctx.client_ip {
            ip.to_string()
        } else {
            let ip = util::get_client_ip(session);
            ctx.client_ip = Some(ip.clone());
            ip
        };

        let found = match self.ip_rules.matched(&ip) {
            Ok(matched) => matched,
            Err(e) => {
                return Ok(Some(HttpResponse::bad_request(
                    e.to_string().into(),
                )));
            },
        };

        // deny ip
        let allow = if self.restriction_category == "deny" {
            !found
        } else {
            found
        };
        if !allow {
            return Ok(Some(self.forbidden_resp.clone()));
        }
        return Ok(None);
    }
}

#[cfg(test)]
mod tests {
    use super::IpRestriction;
    use crate::state::State;
    use crate::{config::PluginConf, config::PluginStep, plugin::Plugin};
    use http::StatusCode;
    use pingora::proxy::Session;
    use pretty_assertions::assert_eq;
    use tokio_test::io::Builder;

    #[test]
    fn test_ip_limit_params() {
        let params = IpRestriction::try_from(
            &toml::from_str::<PluginConf>(
                r###"
ip_list = [
    "192.168.1.1",
    "10.1.1.1",
    "1.1.1.0/24",
    "2.1.1.0/24",
]
type = "deny"
"###,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!("request", params.plugin_step.to_string());
        assert_eq!(
            r#"IpRules { ip_net_list: [1.1.1.0/24, 2.1.1.0/24], ip_list: ["192.168.1.1", "10.1.1.1"] }"#,
            format!("{:?}", params.ip_rules)
        );

        let result = IpRestriction::try_from(
            &toml::from_str::<PluginConf>(
                r###"
step = "response"
ip_list = [
    "192.168.1.1",
    "10.1.1.1",
    "1.1.1.0/24",
    "2.1.1.0/24",
]
type = "deny"
"###,
            )
            .unwrap(),
        );
        assert_eq!("Plugin ip_restriction invalid, message: Ip restriction plugin should be executed at request or proxy upstream step", result.err().unwrap().to_string());
    }

    #[tokio::test]
    async fn test_ip_limit() {
        let deny = IpRestriction::new(
            &toml::from_str::<PluginConf>(
                r###"
type = "deny"
ip_list = [
    "192.168.1.1",
    "1.1.1.0/24",
]
    "###,
            )
            .unwrap(),
        )
        .unwrap();

        let headers = ["X-Forwarded-For: 2.1.1.2"].join("\r\n");
        let input_header =
            format!("GET /vicanso/pingap?size=1 HTTP/1.1\r\n{headers}\r\n\r\n");
        let mock_io = Builder::new().read(input_header.as_bytes()).build();
        let mut session = Session::new_h1(Box::new(mock_io));
        session.read_request().await.unwrap();

        let result = deny
            .handle_request(
                PluginStep::Request,
                &mut session,
                &mut State::default(),
            )
            .await
            .unwrap();
        assert_eq!(true, result.is_none());

        let headers = ["X-Forwarded-For: 192.168.1.1"].join("\r\n");
        let input_header =
            format!("GET /vicanso/pingap?size=1 HTTP/1.1\r\n{headers}\r\n\r\n");
        let mock_io = Builder::new().read(input_header.as_bytes()).build();
        let mut session = Session::new_h1(Box::new(mock_io));
        session.read_request().await.unwrap();

        let result = deny
            .handle_request(
                PluginStep::Request,
                &mut session,
                &mut State::default(),
            )
            .await
            .unwrap();
        assert_eq!(true, result.is_some());

        let headers = ["Accept-Encoding: gzip"].join("\r\n");
        let input_header =
            format!("GET /vicanso/pingap?size=1 HTTP/1.1\r\n{headers}\r\n\r\n");
        let mock_io = Builder::new().read(input_header.as_bytes()).build();
        let mut session = Session::new_h1(Box::new(mock_io));
        session.read_request().await.unwrap();

        let result = deny
            .handle_request(
                PluginStep::Request,
                &mut session,
                &mut State {
                    client_ip: Some("2.1.1.2".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(true, result.is_none());

        let result = deny
            .handle_request(
                PluginStep::Request,
                &mut session,
                &mut State {
                    client_ip: Some("1.1.1.2".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(true, result.is_some());
        assert_eq!(StatusCode::FORBIDDEN, result.unwrap().status);

        let allow = IpRestriction::new(
            &toml::from_str::<PluginConf>(
                r###"
type = "allow"
ip_list = [
    "192.168.1.1",
    "1.1.1.0/24",
]
    "###,
            )
            .unwrap(),
        )
        .unwrap();
        let headers = ["X-Forwarded-For: 192.168.1.1"].join("\r\n");
        let input_header =
            format!("GET /vicanso/pingap?size=1 HTTP/1.1\r\n{headers}\r\n\r\n");
        let mock_io = Builder::new().read(input_header.as_bytes()).build();
        let mut session = Session::new_h1(Box::new(mock_io));
        session.read_request().await.unwrap();

        let result = allow
            .handle_request(
                PluginStep::Request,
                &mut session,
                &mut State::default(),
            )
            .await
            .unwrap();
        assert_eq!(true, result.is_none());
    }
}
