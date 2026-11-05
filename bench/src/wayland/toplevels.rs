use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::backend::{ObjectId, WaylandError};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::{
    self, ExtForeignToplevelHandleV1,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::{
    self, ExtForeignToplevelListV1,
};

/// A toplevel as long as it is listed. Unlike protocol ids, never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ToplevelId(pub u64);

/// What the compositor told the bench, stamped when it arrived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToplevelEvent {
    /// A toplevel is listed with this app_id, or its app_id changed.
    Identified {
        toplevel: ToplevelId,
        app_id: String,
        at: Instant,
    },
    Closed {
        toplevel: ToplevelId,
    },
}

struct Tracked {
    id: ToplevelId,
    app_id: String,
    announced: Option<String>,
}

#[derive(Default)]
struct ListState {
    toplevels: HashMap<ObjectId, Tracked>,
    next_id: u64,
    events: Vec<ToplevelEvent>,
}

/// The compositor's `ext_foreign_toplevel_list_v1`, which names every window.
pub struct Toplevels {
    queue: EventQueue<ListState>,
    state: ListState,
    _list: ExtForeignToplevelListV1,
}

impl Toplevels {
    /// Connects to `$WAYLAND_DISPLAY` and reads the toplevels open now.
    pub fn connect() -> Result<Self> {
        Self::on(&Connection::connect_to_env().context("no Wayland display")?)
    }

    fn on(connection: &Connection) -> Result<Self> {
        let (globals, mut queue) = registry_queue_init::<ListState>(connection)
            .context("could not read the Wayland globals")?;
        let list = globals
            .bind::<ExtForeignToplevelListV1, _, _>(&queue.handle(), 1..=1, ())
            .context("the compositor offers no ext_foreign_toplevel_list_v1 to this client")?;
        let mut state = ListState::default();
        queue.roundtrip(&mut state)?;
        Ok(Self {
            queue,
            state,
            _list: list,
        })
    }

    /// Brings the list up to date.
    pub fn sync(&mut self) -> Result<()> {
        self.queue.roundtrip(&mut self.state)?;
        Ok(())
    }

    /// Handles what arrived, waiting up to `timeout` for something to.
    pub fn dispatch(&mut self, timeout: Duration) -> Result<()> {
        self.queue.flush()?;
        if self.queue.dispatch_pending(&mut self.state)? > 0 {
            return Ok(());
        }
        if let Some(guard) = self.queue.prepare_read() {
            let readable = {
                let fd = guard.connection_fd();
                let mut fds = [PollFd::new(&fd, PollFlags::IN)];
                match poll(&mut fds, Some(&Timespec::try_from(timeout)?)) {
                    Ok(ready) => ready > 0,
                    Err(rustix::io::Errno::INTR) => false,
                    Err(error) => return Err(error.into()),
                }
            };
            if readable {
                match guard.read() {
                    Ok(_) => {}
                    Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        self.queue.dispatch_pending(&mut self.state)?;
        Ok(())
    }

    /// Events since the last call.
    pub fn take_events(&mut self) -> Vec<ToplevelEvent> {
        std::mem::take(&mut self.state.events)
    }

    pub fn open_app_ids(&self) -> impl Iterator<Item = &str> {
        self.state
            .toplevels
            .values()
            .filter_map(|toplevel| toplevel.announced.as_deref())
    }

    /// Android apps with at least one window.
    pub fn android_apps(&self) -> u32 {
        let packages: HashSet<&str> = self
            .open_app_ids()
            .filter_map(android_protocol::package_of)
            .collect();
        u32::try_from(packages.len()).unwrap_or(u32::MAX)
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for ListState {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for ListState {
    fn event(
        state: &mut Self,
        _: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
            let id = ToplevelId(state.next_id);
            state.next_id += 1;
            let tracked = Tracked {
                id,
                app_id: String::new(),
                announced: None,
            };
            state.toplevels.insert(toplevel.id(), tracked);
        }
    }

    event_created_child!(ListState, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for ListState {
    fn event(
        state: &mut Self,
        handle: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let at = Instant::now();
        match event {
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                if let Some(tracked) = state.toplevels.get_mut(&handle.id()) {
                    tracked.app_id = app_id;
                }
            }
            ext_foreign_toplevel_handle_v1::Event::Done => {
                let Some(tracked) = state.toplevels.get_mut(&handle.id()) else {
                    return;
                };
                if tracked.announced.as_ref() != Some(&tracked.app_id) {
                    tracked.announced = Some(tracked.app_id.clone());
                    state.events.push(ToplevelEvent::Identified {
                        toplevel: tracked.id,
                        app_id: tracked.app_id.clone(),
                        at,
                    });
                }
            }
            ext_foreign_toplevel_handle_v1::Event::Closed => {
                if let Some(tracked) = state.toplevels.remove(&handle.id()) {
                    state.events.push(ToplevelEvent::Closed {
                        toplevel: tracked.id,
                    });
                }
                handle.destroy();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::sync::mpsc::{self, Receiver};
    use std::thread;

    use wayland_protocols::ext::foreign_toplevel_list::v1::server::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1 as ServerHandle;
    use wayland_protocols::ext::foreign_toplevel_list::v1::server::ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1 as ServerList;
    use wayland_server::backend::ClientData;
    use wayland_server::{Client, DataInit, DisplayHandle, GlobalDispatch, New, Resource};

    use super::*;

    const WAIT: Duration = Duration::from_secs(5);
    const SERVER_POLL: Duration = Duration::from_millis(1);

    enum Command {
        Map(&'static str),
        Close(usize),
        Quit,
    }

    /// A compositor that lists the toplevels the test maps, announcing those
    /// already mapped to a list as it is bound, as smithay's does.
    #[derive(Default)]
    struct Compositor {
        lists: Vec<ServerList>,
        app_ids: Vec<&'static str>,
        toplevels: Vec<Vec<ServerHandle>>,
    }

    impl Compositor {
        fn map(&mut self, display: &DisplayHandle, app_id: &'static str) {
            let index = self.app_ids.len();
            let handles = self
                .lists
                .iter()
                .filter_map(|list| announce(display, list, index, app_id))
                .collect();
            self.app_ids.push(app_id);
            self.toplevels.push(handles);
        }
    }

    fn announce(
        display: &DisplayHandle,
        list: &ServerList,
        index: usize,
        app_id: &str,
    ) -> Option<ServerHandle> {
        let handle = list
            .client()?
            .create_resource::<ServerHandle, (), Compositor>(display, list.version(), ())
            .ok()?;
        list.toplevel(&handle);
        handle.identifier(format!("toplevel-{index}"));
        handle.app_id(app_id.to_owned());
        handle.done();
        Some(handle)
    }

    impl GlobalDispatch<ServerList, ()> for Compositor {
        fn bind(
            state: &mut Self,
            display: &DisplayHandle,
            _: &Client,
            resource: New<ServerList>,
            _: &(),
            data_init: &mut DataInit<'_, Self>,
        ) {
            let list = data_init.init(resource, ());
            for (index, app_id) in state.app_ids.iter().enumerate() {
                if let Some(handle) = announce(display, &list, index, app_id) {
                    state.toplevels[index].push(handle);
                }
            }
            state.lists.push(list);
        }
    }

    impl wayland_server::Dispatch<ServerList, ()> for Compositor {
        fn request(
            _: &mut Self,
            _: &Client,
            _: &ServerList,
            _: <ServerList as Resource>::Request,
            _: &(),
            _: &DisplayHandle,
            _: &mut DataInit<'_, Self>,
        ) {
        }
    }

    impl wayland_server::Dispatch<ServerHandle, ()> for Compositor {
        fn request(
            _: &mut Self,
            _: &Client,
            _: &ServerHandle,
            _: <ServerHandle as Resource>::Request,
            _: &(),
            _: &DisplayHandle,
            _: &mut DataInit<'_, Self>,
        ) {
        }
    }

    struct TestClient;

    impl ClientData for TestClient {}

    fn serve(stream: UnixStream, commands: Receiver<Command>) -> std::io::Result<()> {
        let mut display =
            wayland_server::Display::<Compositor>::new().map_err(std::io::Error::other)?;
        let handle = display.handle();
        handle.create_global::<Compositor, ServerList, ()>(1, ());
        display
            .handle()
            .insert_client(stream, Arc::new(TestClient))?;
        let mut compositor = Compositor::default();
        loop {
            display.dispatch_clients(&mut compositor)?;
            while let Ok(command) = commands.try_recv() {
                match command {
                    Command::Map(app_id) => compositor.map(&handle, app_id),
                    Command::Close(index) => compositor.toplevels[index]
                        .iter()
                        .for_each(ServerHandle::closed),
                    Command::Quit => return Ok(()),
                }
            }
            display.flush_clients()?;
            thread::sleep(SERVER_POLL);
        }
    }

    fn wait_for_events(toplevels: &mut Toplevels, count: usize) -> Result<Vec<ToplevelEvent>> {
        let started = Instant::now();
        let mut events = Vec::new();
        while events.len() < count && started.elapsed() < WAIT {
            toplevels.dispatch(Duration::from_millis(20))?;
            events.extend(toplevels.take_events());
        }
        Ok(events)
    }

    #[test]
    fn follows_toplevels_of_a_compositor() -> Result<()> {
        let (client_end, server_end) = UnixStream::pair()?;
        let (commands, received) = mpsc::channel();
        commands.send(Command::Map("org.crownos.files"))?;
        let server = thread::spawn(move || serve(server_end, received));

        let mut toplevels = Toplevels::on(&Connection::from_socket(client_end)?)?;
        let initial = wait_for_events(&mut toplevels, 1)?;
        assert!(
            matches!(&initial[..], [ToplevelEvent::Identified { app_id, .. }] if app_id == "org.crownos.files")
        );

        commands.send(Command::Map("waydroid.com.android.settings"))?;
        commands.send(Command::Map("waydroid.com.android.settings"))?;
        let mapped = wait_for_events(&mut toplevels, 2)?;
        let ids: Vec<ToplevelId> = mapped
            .iter()
            .filter_map(|event| match event {
                ToplevelEvent::Identified {
                    toplevel, app_id, ..
                } if app_id == "waydroid.com.android.settings" => Some(*toplevel),
                _ => None,
            })
            .collect();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
        assert_eq!(toplevels.android_apps(), 1);

        commands.send(Command::Close(1))?;
        let closed = wait_for_events(&mut toplevels, 1)?;
        assert!(
            matches!(&closed[..], [ToplevelEvent::Closed { toplevel, .. }] if *toplevel == ids[0])
        );
        assert_eq!(toplevels.open_app_ids().count(), 2);

        commands.send(Command::Quit)?;
        server
            .join()
            .map_err(|_| anyhow::anyhow!("the compositor panicked"))??;
        Ok(())
    }
}
