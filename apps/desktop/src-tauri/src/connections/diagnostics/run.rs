// SPDX-License-Identifier: Apache-2.0
//! Running Test connection: build the route (proxy, a throwaway SSH tunnel),
//! connect with the driver's trace, probe latency, then tear it all down.
use std::sync::Arc;

use openrows_driver_api::Endpoint;
use openrows_net::{Destination, SharedTunnel, TunnelDialer};

use super::super::Connections;
use super::super::error::{IpcError, IpcErrorKind};
use super::super::route::{destination, through_proxy};
use super::super::target::{self, ServerDto, Target};
use super::super::tunnels::route_spec;
use super::TestPlan;
use super::probe::{self, CertificateDto, LatencyDto};
use super::steps::{DatabaseRun, Steps};

/// What a test produced besides its steps.
pub(super) struct Outcome {
    pub(super) result: Result<ServerDto, IpcError>,
    pub(super) certificate: Option<CertificateDto>,
    pub(super) latency: Option<LatencyDto>,
}

impl Outcome {
    fn failed(error: IpcError) -> Self {
        Self {
            result: Err(error),
            certificate: None,
            latency: None,
        }
    }
}

/// The routed target and how it is reached, for step labels.
struct Routed {
    target: Target,
    tunnel: Option<Arc<SharedTunnel>>,
    /// The Unix socket on the SSH server, when the tunnel ends in one.
    socket: Option<String>,
    /// The proxy carrying a direct (non-SSH) connection.
    proxy: Option<String>,
}

/// Route, connect and probe; the throwaway tunnel is closed afterwards.
pub(super) async fn run(
    state: &Connections,
    plan: TestPlan,
    target: Target,
    steps: &mut Steps,
) -> Outcome {
    let routed = match route(state, plan, target, steps).await {
        Ok(routed) => routed,
        Err(error) => return Outcome::failed(error),
    };
    let outcome = database(&routed, steps).await;
    let Routed { target, tunnel, .. } = routed;
    // The target's dialer shares the tunnel; release it so the tunnel can close.
    drop(target);
    if let Some(tunnel) = tunnel {
        tunnel.close().await;
    }
    outcome
}

async fn route(
    state: &Connections,
    plan: TestPlan,
    target: Target,
    steps: &mut Steps,
) -> Result<Routed, IpcError> {
    let TestPlan {
        profile,
        ssh: secrets,
        proxy,
        ..
    } = plan;
    if let Some(hop) = &profile.helper {
        let started = std::time::Instant::now();
        let result = super::super::helpers::acquire(state, &profile).await;
        steps.helper(hop, &result, started.elapsed());
        let helper = result?;
        return Ok(Routed {
            target: target.through(Arc::new(openrows_net::helper::HelperDialer::new(helper))),
            tunnel: None,
            socket: None,
            proxy: Some(hop.describe()),
        });
    }
    let Some(ssh) = &profile.ssh else {
        let described = proxy.as_ref().map(|proxy| proxy.describe());
        let target = match proxy {
            Some(proxy) => through_proxy(target, proxy, &profile),
            None => target,
        };
        return Ok(Routed {
            target,
            tunnel: None,
            socket: None,
            proxy: described,
        });
    };
    let spec = route_spec(ssh, secrets)?;
    let options = state.opener.options(proxy);
    let attempt = state
        .opener
        .open(state.opener.plan(&spec).await?, &options)
        .await;
    if let Some(error) = steps.ssh(&attempt) {
        return Err(error);
    }
    let shared = Arc::new(attempt.shared(options)?);
    let destination = destination(&profile);
    let socket = match &destination {
        Destination::Socket(path) => Some(path.clone()),
        Destination::Tcp { .. } => None,
    };
    let dialer = TunnelDialer::new(Arc::clone(&shared), destination);
    Ok(Routed {
        target: target.through(Arc::new(dialer)),
        tunnel: Some(shared),
        socket,
        proxy: None,
    })
}

async fn database(routed: &Routed, steps: &mut Steps) -> Outcome {
    let (result, trace) = target::connect_traced(&routed.target).await;
    let certificate = probe::certificate(&trace);
    let server = result
        .as_ref()
        .ok()
        .map(|connection| ServerDto::from(connection.server()));
    let request = &routed.target.request;
    let (host, port, file) = match &request.endpoint {
        Endpoint::Server { host, port } => (host.clone(), *port, false),
        Endpoint::File { source, .. } => (source.describe(), 0, true),
    };
    let run = DatabaseRun {
        trace: &trace,
        host: &host,
        port,
        user: &request.user,
        tunnelled: routed.tunnel.is_some(),
        socket: routed.socket.as_deref(),
        proxy: routed.proxy.as_deref(),
        file,
        server: server.as_ref(),
    };
    steps.database(&run, result.as_ref().err());
    let (mut connection, server) = match (result, server) {
        (Ok(connection), Some(server)) => (connection, server),
        (Err(error), _) => return outcome(Err(IpcError::from(&error)), certificate, None),
        (Ok(_), None) => {
            let error = IpcError::new(IpcErrorKind::Internal, "The connection reported no server");
            return outcome(Err(error), certificate, None);
        }
    };
    let latency = probe::latency(connection.as_mut())
        .await
        .map_err(|error| IpcError::from(&error));
    steps.latency(&latency);
    // The test already succeeded; a failed goodbye changes nothing.
    let _ = connection.close().await;
    outcome(Ok(server), certificate, latency.ok())
}

fn outcome(
    result: Result<ServerDto, IpcError>,
    certificate: Option<CertificateDto>,
    latency: Option<LatencyDto>,
) -> Outcome {
    Outcome {
        result,
        certificate,
        latency,
    }
}
