use anyhow::{Context, bail};
use backoff::{Error as BackoffError, future::retry};
use futures::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

use super::LlmClient;

include!("streaming.rs");
include!("generation.rs");
include!("anthropic.rs");
include!("types.rs");
