use wayland_client::{protocol::{wl_compositor, wl_shm, wl_shm_pool, wl_buffer, wl_surface, wl_callback}, Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};
use std::time::{Duration, Instant};
use tempfile::tempfile;
use std::os::unix::io::AsRawFd;
use std::io::{Write, Seek};

use std::fs::File;
use std::path::PathBuf;
// use gif::{DecodeOptions}; // Removed: No longer used directly in main.rs

mod gif_processor;

// Struct to hold Wayland objects
struct State {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    surface: Option<wl_surface::WlSurface>,
    xdg_surface: Option<xdg_surface::XdgSurface>,
    xdg_toplevel: Option<xdg_toplevel::XdgToplevel>,
    gif_data: Option<gif_processor::GifData>, // Encapsulated GIF data
    current_frame_index: usize,
    // gif_width, gif_height, gif_frames_rgba, gif_frame_delays are now in gif_data
    shm_pool: Option<wl_shm_pool::WlShmPool>, // Corrected type
    current_buffer: Option<wl_buffer::WlBuffer>,
    needs_redraw: bool,
    last_frame_time: Option<Instant>,
    current_shm_temp_file: Option<File>, // Changed from tempfile::TempFile to std::fs::File
    frame_callback: Option<wl_callback::WlCallback>, // For wl_surface.frame
}

impl State {
    fn new() -> Self {
        State {
            compositor: None,
            shm: None,
            wm_base: None,
            surface: None,
            xdg_surface: None,
            xdg_toplevel: None,
            gif_data: None,
            current_frame_index: 0,
            shm_pool: None,
            current_buffer: None,
            needs_redraw: true, // Start with a redraw request
            last_frame_time: None,
            current_shm_temp_file: None,
            frame_callback: None,
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for State {
    fn event(
        state: &mut Self,
        _callback: &wl_callback::WlCallback, // Prefixed with _
        event: wl_callback::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_callback::Event::Done { callback_data } => {
                println!("Frame callback done (data: {}) - compositor ready for next frame.", callback_data);
                // We don't set needs_redraw here. needs_redraw is set when content *changes*.
                // This callback just signals that the compositor is ready for a new commit.
                state.frame_callback = None;
            }
            _ => {} // Should not happen for wl_callback
        }
    }
}

// Implement Dispatch for Wayland events
impl Dispatch<wl_compositor::WlCompositor, ()> for State {
    fn event(
        _state: &mut Self, // unused
        _: &wl_compositor::WlCompositor,
        event: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Compositor events are not handled in this example
        println!("Compositor event: {:?}", event);
    }
}

impl Dispatch<wl_shm::WlShm, ()> for State {
    fn event(
        _state: &mut Self, // unused
        _: &wl_shm::WlShm,
        event: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // SHM events are not handled in this example
        println!("SHM event: {:?}", event);
    }
}

// Dispatch for WlShmPool
impl Dispatch<wl_shm_pool::WlShmPool, ()> for State {
    fn event(
        _state: &mut Self,
        _pool: &wl_shm_pool::WlShmPool,
        event: wl_shm_pool::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>
    ) {
        // For wl_shm_pool, there are no events defined in the protocol,
        // so this handler should ideally not be called.
        // However, implementing Dispatch is necessary for object creation.
        println!("wl_shm_pool event: {:?}", event);
    }
}


impl Dispatch<xdg_wm_base::XdgWmBase, ()> for State {
    fn event(
        _state: &mut Self, // unused
        wm_base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_wm_base::Event::Ping { serial } => {
                wm_base.pong(serial);
                println!("XDG WM Base: Ping/Pong");
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &wl_surface::WlSurface,
        _event: wl_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>
    ) {
        // Surface events like enter/leave are not critical for a simple viewer yet
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for State {
    fn event(
        state: &mut Self,
        xdg_surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>
    ) {
        match event {
            xdg_surface::Event::Configure { serial, .. } => {
                xdg_surface.ack_configure(serial);
                state.needs_redraw = true;
                println!("XDG Surface: Configure, needs_redraw set to true");
            }
            _ => {}
        }
    }
}

// Placeholder for wl_buffer events if needed later
impl Dispatch<wl_buffer::WlBuffer, ()> for State {
    fn event(
        _state: &mut Self,
        _buffer: &wl_buffer::WlBuffer,
        _event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>
    ) {
        // For example, handle release events to reuse buffers
    }
}


impl Dispatch<xdg_toplevel::XdgToplevel, ()> for State {
    fn event(
        _state: &mut Self,
        _toplevel: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, states } => {
                println!(
                    "XDG Toplevel Configure: width {}, height {}, states: {:?}",
                    width, height, states
                );
            }
            xdg_toplevel::Event::Close => {
                println!("XDG Toplevel: Close event. Exiting.");
                // In a real app, you'd signal the event loop to exit here.
                std::process::exit(0);
            }
            _ => {}
        }
    }
}


fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: papers <path_to_gif>");
        std::process::exit(1);
    }
    let gif_path = PathBuf::from(&args[1]);

    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland display");
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    let display = conn.display();
    let mut state = State::new();

    // Load GIF using the new processor
    match gif_processor::GifData::new(&gif_path) {
        Ok(gif_data) => {
            println!("GIF loaded successfully: {}x{} with {} frames.", gif_data.width, gif_data.height, gif_data.frames_bgra.len());
            state.gif_data = Some(gif_data);
        }
        Err(e) => {
            eprintln!("Failed to load GIF: {}", e);
            std::process::exit(1);
        }
    }

    let _registry = display.get_registry(&qh, ()); // Prefixed with _

    // This roundtrip waits for the server to process requests and send back events,
    // ensuring that the globals are available.
    event_queue.roundtrip(&mut state).expect("Failed initial roundtrip");

    // Bind to globals. We need to re-dispatch after binding.
    // The Dispatch implementations for wl_registry will populate our State struct.
    event_queue.roundtrip(&mut state).expect("Failed roundtrip after binding globals");


    let compositor = state.compositor.as_ref().expect("Wayland compositor not available");
    let _shm = state.shm.as_ref().expect("Wayland SHM not available"); // Prefixed with _
    let wm_base = state.wm_base.as_ref().expect("XDG WM Base not available");

    let surface = compositor.create_surface(&qh, ());
    state.surface = Some(surface.clone());

    let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
    state.xdg_surface = Some(xdg_surface.clone());

    let xdg_toplevel = xdg_surface.get_toplevel(&qh, ());
    state.xdg_toplevel = Some(xdg_toplevel.clone());

    xdg_toplevel.set_title("Paper GIF Viewer".to_string());
    // Initial commit to make the surface known to the compositor
    // We will commit after the first draw.
    // surface.commit();
    if let Some(gif_data) = state.gif_data.as_ref() {
        println!("Wayland setup complete. Window created. GIF loaded with {} frames.", gif_data.frames_bgra.len());
    } else {
        println!("Wayland setup complete. Window created. No GIF data loaded (should have exited if error).");
    }

    // Ensure the window is configured before first draw
    // Dispatch events once to process initial configure from compositor
    event_queue.roundtrip(&mut state).expect("Failed roundtrip for initial configure");


    // Event loop for animation
    loop {
        // Process any pending Wayland events without blocking indefinitely.
        // This is important so we can advance animation frames.
        if event_queue.dispatch_pending(&mut state).is_err() {
            eprintln!("Error in dispatch_pending. Exiting.");
            break;
        }

        // Redraw if needed (e.g., due to configure event or animation tick)
        if state.needs_redraw {
            if let Err(e) = draw_frame(&mut state, &qh) {
                eprintln!("Error drawing frame: {}", e);
                // Potentially break or handle error appropriately
            }
            // The draw_frame function might set needs_redraw for the next animation frame.
            // For now, we set it to false here, and animation logic will set it true.
            // No, draw_frame itself doesn't set needs_redraw. The animation logic below will.
            // state.needs_redraw = false; // This is now handled carefully
        }

        let now = Instant::now(); // 'now' should be captured once at the start of all logic for this iteration.

        // 1. GIF Animation Logic: Determine if the GIF frame should advance
        if let Some(gif_data) = state.gif_data.as_ref() {
            if !gif_data.frames_bgra.is_empty() {
                if let Some(last_frame_event_time) = state.last_frame_time {
                    let current_delay_centis = gif_data.frame_delays[state.current_frame_index];
                    let current_frame_target_duration_ms = if current_delay_centis == 0 { 100 } else { current_delay_centis as u64 * 10 };
                    let current_frame_target_duration = Duration::from_millis(current_frame_target_duration_ms);

                    if now.duration_since(last_frame_event_time) >= current_frame_target_duration {
                        state.current_frame_index = (state.current_frame_index + 1) % gif_data.frames_bgra.len();
                        state.needs_redraw = true;
                        state.last_frame_time = Some(now); // Time is updated when we DECIDE to advance frame
                    }
                } else {
                    // This is for the very first frame. needs_redraw is true from State::new().
                    state.last_frame_time = Some(now);
                    state.needs_redraw = true; // Ensure it's still true
                }
            }
        }


        // 2. Drawing Logic: Draw if needed and if compositor is ready (frame_callback is None)
        if state.needs_redraw && state.frame_callback.is_none() {
            if let Err(e) = draw_frame(&mut state, &qh) { // draw_frame will request a new frame_callback
                eprintln!("Error drawing frame: {}", e);
            }
            state.needs_redraw = false; // Redraw request has been handled
        }

        // 3. Sleep Logic
        let mut sleep_duration;
        if state.frame_callback.is_some() {
            // Waiting for compositor (frame callback is pending), so short sleep.
            sleep_duration = Duration::from_millis(1);
        } else if let Some(gif_data) = state.gif_data.as_ref() {
            if !gif_data.frames_bgra.is_empty() {
                if let Some(last_frame_display_time) = state.last_frame_time {
                    let current_delay_centis = gif_data.frame_delays[state.current_frame_index];
                    let frame_duration_ms = if current_delay_centis == 0 { 100 } else { current_delay_centis as u64 * 10 };
                    let frame_target_duration = Duration::from_millis(frame_duration_ms);

                    let time_since_last_frame_drawn = now.duration_since(last_frame_display_time);

                    if time_since_last_frame_drawn < frame_target_duration {
                        sleep_duration = frame_target_duration - time_since_last_frame_drawn;
                    } else {
                        sleep_duration = Duration::from_millis(1);
                    }
                } else {
                    sleep_duration = Duration::from_millis(1); // Wait for first callback / first draw to set time
                }
            } else {
                 sleep_duration = Duration::from_millis(100); // No frames in GIF data
            }
        } else {
            // No GIF data loaded, default sleep.
            sleep_duration = Duration::from_millis(100);
        }

        sleep_duration = sleep_duration.max(Duration::from_millis(1)); // Ensure minimum sleep.
        std::thread::sleep(sleep_duration);

        // Ensure the connection is flushed, sending requests to the server.
        // dispatch_pending alone does not guarantee a flush.
        // A blocking_dispatch or roundtrip would flush, but we want to control timing.
        if conn.flush().is_err() {
            eprintln!("Failed to flush Wayland connection. Exiting.");
            break;
        }
    }
}

fn draw_frame(state: &mut State, qh: &QueueHandle<State>) -> Result<(), String> {
    let surface = state.surface.as_ref().ok_or("Surface not initialized")?;
    let shm = state.shm.as_ref().ok_or("SHM not initialized")?;

    let gif_data = state.gif_data.as_ref().ok_or("GIF data not loaded")?;

    if gif_data.frames_bgra.is_empty() {
        return Err("No GIF frames to draw".to_string());
    }
    if state.current_frame_index >= gif_data.frames_bgra.len() {
         return Err(format!("current_frame_index {} out of bounds for gif_frames_bgra (len {})", state.current_frame_index, gif_data.frames_bgra.len()));
    }

    let frame_rgba_data = &gif_data.frames_bgra[state.current_frame_index];
    if frame_rgba_data.is_empty() {
        // This could happen if a frame had no palette and we skipped it.
        println!("Skipping draw for empty frame_rgba_data at index {}", state.current_frame_index);
        return Ok(());
    }

    let width = gif_data.width as i32;
    let height = gif_data.height as i32;
    let stride = width * 4; // 4 bytes per pixel (BGRA)
    let size = stride * height;

    // Ensure SHM pool and its backing file are created if they don't exist
    if state.shm_pool.is_none() {
        let temp_file = tempfile().map_err(|e| format!("Failed to create temp file: {}", e))?;
        state.current_shm_temp_file = Some(temp_file);

        let pool_fd = state.current_shm_temp_file.as_ref().unwrap().as_raw_fd();
        let pool = shm.create_pool(pool_fd, size, qh, ());
        state.shm_pool = Some(pool);
        println!("Created new SHM pool and temp_file.");
    }

    // Get the current temp_file to write into.
    // We need to seek to the beginning before writing each frame's data.
    let current_temp_file = state.current_shm_temp_file.as_mut()
        .ok_or("SHM temp_file not available even after creation attempt")?;

    current_temp_file.seek(std::io::SeekFrom::Start(0))
        .map_err(|e| format!("Failed to seek to start of temp file: {}", e))?;
    current_temp_file.write_all(frame_rgba_data)
        .map_err(|e| format!("Failed to write to temp file: {}", e))?;
    current_temp_file.flush()
        .map_err(|e| format!("Failed to flush temp file: {}", e))?;

    // Destroy old buffer before creating a new one from the (potentially same) pool
    if let Some(old_buffer) = state.current_buffer.take() {
        old_buffer.destroy();
    }

    let pool = state.shm_pool.as_ref().unwrap(); // Should exist now
    let buffer = pool.create_buffer(0, width, height, stride, wl_shm::Format::Argb8888, qh, ());
    state.current_buffer = Some(buffer.clone());


    // 5. Attach the buffer to the surface and commit
    surface.attach(Some(&buffer), 0, 0);
    surface.damage_buffer(0, 0, width, height); // Mark the entire buffer as damaged

    // Request a frame callback if one isn't already pending
    if state.frame_callback.is_none() {
        let callback = surface.frame(qh, ());
        state.frame_callback = Some(callback);
    }

    surface.commit();

    println!("Frame {} drawn. Width: {}, Height: {}", state.current_frame_index, width, height);

    // The responsibility of advancing frames is moved to the main loop's animation logic.
    Ok(())
}

// We need a Dispatch implementation for wl_registry to get global objects
impl Dispatch<wayland_client::protocol::wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wayland_client::protocol::wl_registry::WlRegistry,
        event: wayland_client::protocol::wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wayland_client::protocol::wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            println!("Found global: name={}, interface={}, version={}", name, interface, version);
            match interface.as_str() {
                "wl_compositor" => {
                    let compositor = registry.bind::<wl_compositor::WlCompositor, _, _>(name, version, qh, ());
                    state.compositor = Some(compositor);
                }
                "wl_shm" => {
                    let shm = registry.bind::<wl_shm::WlShm, _, _>(name, version, qh, ());
                    state.shm = Some(shm);
                }
                "xdg_wm_base" => {
                    let wm_base = registry.bind::<xdg_wm_base::XdgWmBase, _, _>(name, version, qh, ());
                    state.wm_base = Some(wm_base);
                }
                _ => {}
            }
        }
    }
}
