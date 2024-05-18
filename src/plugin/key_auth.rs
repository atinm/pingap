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
    get_int_conf, get_step_conf, get_str_conf, get_str_slice_conf, Error, ProxyPlugin, Result,
};
use crate::config::{PluginCategory, PluginConf, PluginStep};
use crate::http_extra::HttpResponse;
use crate::state::State;
use crate::util;
use async_trait::async_trait;
use bytes::Bytes;
use http::{HeaderName, StatusCode};
use log::debug;
use pingora::proxy::Session;
use std::str::FromStr;
use substring::Substring;

pub struct KeyAuth {
    category: u8,
    plugin_step: PluginStep,
    header_name: Option<HeaderName>,
    query_name: Option<String>,
    keys: Vec<Vec<u8>>,
    miss_authorization_resp: HttpResponse,
    unauthorized_resp: HttpResponse,
}

struct KeyAuthParams {
    category: u8,
    plugin_step: PluginStep,
    header_name: Option<HeaderName>,
    query_name: Option<String>,
    keys: Vec<Vec<u8>>,
}

impl TryFrom<&PluginConf> for KeyAuthParams {
    type Error = Error;
    fn try_from(value: &PluginConf) -> Result<Self> {
        let step = get_step_conf(value);
        let all_params = get_str_conf(value, "value");
        let params = if !all_params.is_empty() {
            let arr: Vec<&str> = all_params.split(' ').collect();
            if arr.len() != 2 {
                return Err(Error::Invalid {
                    category: PluginCategory::KeyAuth.to_string(),
                    message: "Value for key auth is invalid".to_string(),
                });
            }
            let mut category = 0;
            let mut query_name = None;
            let mut header_name = None;
            let name = arr[0];
            if name.starts_with('?') {
                category = 1;
                query_name = Some(name.substring(1, name.len()).to_string());
            } else {
                header_name = Some(HeaderName::from_str(name).map_err(|e| Error::Invalid {
                    category: PluginCategory::KeyAuth.to_string(),
                    message: format!("invalid header name, {e}"),
                })?);
            }

            let keys = arr[1]
                .split(',')
                .map(|item| item.as_bytes().to_owned())
                .collect();
            Self {
                category,
                keys,
                plugin_step: step,
                query_name,
                header_name,
            }
        } else {
            let category = get_int_conf(value, "category") as u8;
            let name = get_str_conf(value, "name");
            let mut query_name = None;
            let mut header_name = None;
            if category == 1 {
                query_name = Some(name.substring(1, name.len()).to_string());
            } else {
                header_name = Some(HeaderName::from_str(&name).map_err(|e| Error::Invalid {
                    category: PluginCategory::KeyAuth.to_string(),
                    message: format!("invalid header name, {e}"),
                })?);
            }
            Self {
                category,
                keys: get_str_slice_conf(value, "keys")
                    .iter()
                    .map(|item| item.as_bytes().to_vec())
                    .collect(),
                plugin_step: step,
                query_name,
                header_name,
            }
        };
        if ![PluginStep::Request, PluginStep::ProxyUpstream].contains(&params.plugin_step) {
            return Err(Error::Invalid {
                category: PluginCategory::KeyAuth.to_string(),
                message: "Key auth plugin should be executed at request or proxy upstream step"
                    .to_string(),
            });
        }
        Ok(params)
    }
}

impl KeyAuth {
    pub fn new(params: &PluginConf) -> Result<Self> {
        debug!("new key auth proxy plugin, params:{params:?}");
        let params = KeyAuthParams::try_from(params)?;

        Ok(Self {
            category: params.category,
            keys: params.keys,
            plugin_step: params.plugin_step,
            query_name: params.query_name,
            header_name: params.header_name,
            miss_authorization_resp: HttpResponse {
                status: StatusCode::UNAUTHORIZED,
                body: Bytes::from_static(b"Key missing"),
                ..Default::default()
            },
            unauthorized_resp: HttpResponse {
                status: StatusCode::UNAUTHORIZED,
                body: Bytes::from_static(b"Key auth fail"),
                ..Default::default()
            },
        })
    }
}

#[async_trait]
impl ProxyPlugin for KeyAuth {
    #[inline]
    fn step(&self) -> PluginStep {
        self.plugin_step
    }
    #[inline]
    fn category(&self) -> PluginCategory {
        PluginCategory::KeyAuth
    }
    #[inline]
    async fn handle(
        &self,
        session: &mut Session,
        _ctx: &mut State,
    ) -> pingora::Result<Option<HttpResponse>> {
        let value = if self.category == 0 {
            self.header_name
                .as_ref()
                .map(|v| session.get_header_bytes(v))
        } else {
            self.query_name.as_ref().map(|name| {
                util::get_query_value(session.req_header(), name)
                    .unwrap_or_default()
                    .as_bytes()
            })
        };
        if value.is_none() {
            return Ok(Some(self.miss_authorization_resp.clone()));
        }
        if !self.keys.contains(&value.unwrap().to_vec()) {
            return Ok(Some(self.unauthorized_resp.clone()));
        }
        Ok(None)
    }
}
