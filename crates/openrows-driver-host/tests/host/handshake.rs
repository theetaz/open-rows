// SPDX-License-Identifier: Apache-2.0
//! The real child refuses wrong/missing/replayed peer auth before any Connect.
use crate::support;
use openrows_host_client::{
    LaunchOptions, MemoryBudget, launch_command, transport::Listener, write_bootstrap,
};
use openrows_host_protocol::{
    Bootstrap, Control, Frame, Hello, Lane, LaneHello, WireSecret, read_frame, write_frame,
};
use std::path::Path;
use tokio::process::Child;

struct Fixture {
    child: Child,
    bootstrap: Bootstrap,
    control: Listener,
    data: Listener,
    directory: tempfile::TempDir,
}
impl Fixture {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        support::prepare_image().await?;
        let options: LaunchOptions = support::options(MemoryBudget::default());
        let directory = tempfile::Builder::new()
            .prefix(".h-")
            .tempdir_in(support::root())?;
        let control_path = endpoint(directory.path(), "c");
        let data_path = endpoint(directory.path(), "d");
        let control = Listener::bind(Path::new(&control_path))?;
        let data = Listener::bind(Path::new(&data_path))?;
        let bootstrap = Bootstrap {
            control: control_path,
            data: data_path,
            directory: directory.path().to_string_lossy().into_owned(),
            token: WireSecret::new("test-only".into()),
            parent_generation: 1,
            generation: 2,
        };
        let mut command = launch_command(&options)?;
        assert!(!format!("{command:?}").contains("test-only"));
        let native = command.as_std();
        assert_eq!(native.get_args().collect::<Vec<_>>(), ["sqlite"]);
        assert!(
            native
                .get_args()
                .all(|value| !value.to_string_lossy().contains("test-only"))
        );
        assert!(
            native
                .get_envs()
                .all(|(key, value)| !key.to_string_lossy().contains("test-only")
                    && value.is_none_or(|value| !value.to_string_lossy().contains("test-only")))
        );
        let mut child = command.spawn()?;
        write_bootstrap(&mut child.stdin.take().ok_or("no stdin")?, &bootstrap).await?;
        Ok(Self {
            child,
            bootstrap,
            control,
            data,
            directory,
        })
    }
    async fn lanes(
        &mut self,
    ) -> Result<
        (
            openrows_host_client::transport::Stream,
            openrows_host_client::transport::Stream,
        ),
        Box<dyn std::error::Error>,
    > {
        let mut control = self.control.accept().await?;
        let sent = read_frame(&mut control, Lane::Control)
            .await?
            .decode_control()?;
        assert!(matches!(sent, Control::Authenticate(_)));
        let ack = auth(&self.bootstrap, Lane::Control, "test-only");
        write_frame(&mut control, &Frame::control(2, 0, &ack)?, Lane::Control).await?;
        let mut data = self.data.accept().await?;
        assert!(matches!(
            read_frame(&mut data, Lane::Data).await?.decode_control()?,
            Control::Authenticate(_)
        ));
        let ack = auth(&self.bootstrap, Lane::Data, "test-only");
        write_frame(&mut data, &Frame::control(2, 0, &ack)?, Lane::Data).await?;
        Ok((control, data))
    }
    async fn failed(self) -> Result<(), Box<dyn std::error::Error>> {
        let pid = self.child.id().ok_or("no PID")?;
        let path = self.directory.path().to_path_buf();
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(4),
            self.child.wait_with_output(),
        )
        .await??;
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("test-only"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("test-only"));
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
        support::assert_reaped(pid, &path);
        Ok(())
    }
}
fn endpoint(root: &Path, name: &str) -> String {
    #[cfg(unix)]
    {
        root.join(name).to_string_lossy().into_owned()
    }
    #[cfg(windows)]
    {
        format!(
            r"\\.\pipe\openrows-test-{}-{name}",
            root.file_name().unwrap().to_string_lossy()
        )
    }
}
fn auth(bootstrap: &Bootstrap, lane: Lane, token: &str) -> Control {
    Control::Authenticate(LaneHello {
        token: WireSecret::new(token.into()),
        lane,
        parent_generation: bootstrap.parent_generation,
        generation: bootstrap.generation,
    })
}
#[tokio::test]
async fn wrong_and_missing_secret_rejected_without_secret_in_command_or_logs()
-> Result<(), Box<dyn std::error::Error>> {
    for token in ["wrong", ""] {
        let mut fixture = Fixture::new().await?;
        let mut control = fixture.control.accept().await?;
        read_frame(&mut control, Lane::Control).await?;
        let ack = auth(&fixture.bootstrap, Lane::Control, token);
        write_frame(&mut control, &Frame::control(2, 0, &ack)?, Lane::Control).await?;
        fixture.failed().await?;
    }
    Ok(())
}
#[tokio::test]
async fn replay_and_incompatible_build_rejected_before_credentials()
-> Result<(), Box<dyn std::error::Error>> {
    for replay in [false, true] {
        let mut fixture = Fixture::new().await?;
        let (mut control, _data) = fixture.lanes().await?;
        let dto = if replay {
            auth(&fixture.bootstrap, Lane::Control, "test-only")
        } else {
            let mut hello = Hello::sqlite(2);
            hello.build.push('x');
            Control::Hello(hello)
        };
        write_frame(&mut control, &Frame::control(2, 0, &dto)?, Lane::Control).await?;
        fixture.failed().await?;
    }
    Ok(())
}
