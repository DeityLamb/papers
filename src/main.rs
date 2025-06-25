use wayland_client::{protocol::{wl_compositor, wl_shm, wl_shm_pool, wl_buffer, wl_surface}, Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};
use std::time::{Duration, Instant};
use tempfile::tempfile;
use std::os::unix::io::AsRawFd;
use std::io::{Write, Seek};

use std::fs::File;
use std::path::PathBuf;
use gif::{DecodeOptions}; // Removed Frame, DecodingError, SetParameter, ColorOutput.

// Struct to hold Wayland objects
struct State {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    surface: Option<wl_surface::WlSurface>,
    xdg_surface: Option<xdg_surface::XdgSurface>,
    xdg_toplevel: Option<xdg_toplevel::XdgToplevel>,
    gif_frames_rgba: Vec<Vec<u8>>, // Stores raw RGBA data for each frame
    gif_frame_delays: Vec<u16>, // Stores delay for each frame
    gif_width: u16,
    gif_height: u16,
    current_frame_index: usize,
    shm_pool: Option<wl_shm_pool::WlShmPool>, // Corrected type
    current_buffer: Option<wl_buffer::WlBuffer>,
    needs_redraw: bool,
    last_frame_time: Option<Instant>,
    current_shm_temp_file: Option<File>, // Changed from tempfile::TempFile to std::fs::File
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
            gif_frames_rgba: Vec::new(),
            gif_frame_delays: Vec::new(),
            gif_width: 0,
            gif_height: 0,
            current_frame_index: 0,
            shm_pool: None,
            current_buffer: None,
            needs_redraw: true, // Start with a redraw request
            last_frame_time: None,
            current_shm_temp_file: None,
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

    // Load GIF
    let options = DecodeOptions::new(); // Removed mut
    // No .set() call for color output in gif 0.12.0; manual conversion needed.

    let gif_file = File::open(&gif_path).expect("Failed to open GIF file");
    let mut reader = options.read_info(gif_file).expect("Failed to read GIF info");

    state.gif_width = reader.width();
    state.gif_height = reader.height();
    let palette = reader.global_palette().unwrap_or_default().to_vec(); // Get global palette
    println!("GIF dimensions: {}x{}, Global palette size: {}", state.gif_width, state.gif_height, palette.len() / 3);


    // Loop to read frames
    while let Some(frame) = reader.read_next_frame().expect("Failed to read next frame") {
        let frame_info = frame.to_owned(); // Clone frame metadata (delay, etc.)
        state.gif_frame_delays.push(frame_info.delay);

        let buffer_capacity = (frame_info.width as usize) * (frame_info.height as usize) * 4;
        let mut rgba_buffer = Vec::with_capacity(buffer_capacity);

        // Clear buffer with transparent black (BGRA: 0,0,0,0) before drawing new frame data
        // This helps with GIFs that have transparency or frames smaller than the canvas.
        for _ in 0..(buffer_capacity / 4) {
            rgba_buffer.extend_from_slice(&[0, 0, 0, 0]); // B, G, R, A (transparent)
        }
        // Ensure the buffer is then set to the correct length for direct writing if needed,
        // but extend_from_slice handles length. The previous loop fills it.
        // A more efficient way to fill would be:
        // rgba_buffer.resize(buffer_capacity, 0); // Fills with 0s.
        // However, extend_from_slice is also fine for clarity of BGRA.
        // Let's stick to a loop for clarity of the [0,0,0,0] pattern for now.
        // Actually, a more direct way to fill for this specific case:
        rgba_buffer.clear();
        // Fill with opaque black (BGRA: 0,0,0,255)
        for _ in 0..(buffer_capacity / 4) {
            rgba_buffer.extend_from_slice(&[0, 0, 0, 255]); // B, G, R, Alpha (opaque black)
        }


        // Determine which palette to use: local or global
        let current_palette = frame_info.palette.as_ref().unwrap_or(&palette);

        if current_palette.is_empty() && !frame_info.buffer.is_empty() {
            eprintln!("Warning: Frame has data but no palette (global or local). Skipping frame.");
            // Add a placeholder empty buffer or handle as an error if strict
            state.gif_frames_rgba.push(Vec::new());
            continue;
        }

        // The frame buffer contains indexed color data or RGBA data
        // For gif 0.12.0, `frame.buffer` is `Cow<[u8]>`.
        // If the GIF is already RGBA (e.g. some animated WebP converted to GIF), buffer might be RGBA.
        // However, typical GIFs are paletted. The `gif` crate's default decoding
        // gives indexed data if a palette is present.
        // We previously tried to force RGBA output using `set()`, which isn't available.
        // So, we assume `frame.buffer` is indexed if a palette is available.

        // If frame_info.width * frame_info.height * 4 == frame_info.buffer.len(), it might be RGBA already.
        // But this is not a reliable check from the gif crate itself for 0.12.0 without `set_color_output`.
        // The most robust way is to always use the palette if present.

        let mut pixel_idx = 0;
        for &index in frame_info.buffer.iter() {
            let r: u8;
            let g: u8;
            let b: u8;
            let a: u8 = 255; // Default alpha to opaque

            if (index as usize * 3 + 2) < current_palette.len() {
                r = current_palette[index as usize * 3];
                g = current_palette[index as usize * 3 + 1];
                b = current_palette[index as usize * 3 + 2];
            } else {
                // Index out of bounds for palette, use a default color (e.g., black)
                r = 0; g = 0; b = 0;
            }

            let base = pixel_idx * 4;
            if base + 3 < rgba_buffer.len() {
                 // Swizzle to BGRA for Argb8888 format on little-endian
                rgba_buffer[base] = b;
                rgba_buffer[base + 1] = g;
                rgba_buffer[base + 2] = r;
                rgba_buffer[base + 3] = a;
            }
            pixel_idx += 1;
        }
        state.gif_frames_rgba.push(rgba_buffer);
    }

    println!("Loaded {} frames from GIF.", state.gif_frames_rgba.len());
    if state.gif_frames_rgba.is_empty() {
        eprintln!("No frames found in GIF or failed to process frames.");
        std::process::exit(1);
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

    println!("Wayland setup complete. Window created. GIF loaded with {} frames.", state.gif_frames_rgba.len());

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

        let now = Instant::now();
        let mut calculated_sleep_duration = Duration::from_millis(100); // Default sleep if no frames

        if !state.gif_frames_rgba.is_empty() {
            let current_gif_frame_delay_centis = state.gif_frame_delays[state.current_frame_index];
            let current_gif_frame_duration_ms = if current_gif_frame_delay_centis == 0 { 100 } else { current_gif_frame_delay_centis as u64 * 10 };
            let current_frame_target_duration = Duration::from_millis(current_gif_frame_duration_ms);

            if let Some(last_display_time) = state.last_frame_time {
                let elapsed_since_last_display = now.duration_since(last_display_time);

                if elapsed_since_last_display >= current_frame_target_duration {
                    // Time to advance to the next frame
                    state.current_frame_index = (state.current_frame_index + 1) % state.gif_frames_rgba.len();
                    state.needs_redraw = true; // Mark that this new frame needs drawing
                    state.last_frame_time = Some(now); // Record time for this new frame's display start

                    // Sleep for the new current frame's duration
                    let next_frame_delay_centis = state.gif_frame_delays[state.current_frame_index];
                    let next_frame_duration_ms = if next_frame_delay_centis == 0 { 100 } else { next_frame_delay_centis as u64 * 10 };
                    calculated_sleep_duration = Duration::from_millis(next_frame_duration_ms);
                } else {
                    // Not yet time to advance, sleep for the remainder of current frame's duration
                    calculated_sleep_duration = current_frame_target_duration - elapsed_since_last_display;
                }
            } else {
                // This is the very first frame to be shown
                state.needs_redraw = true; // Mark that this first frame needs drawing
                state.last_frame_time = Some(now); // Record time for this first frame's display start
                calculated_sleep_duration = current_frame_target_duration;
            }
        } else {
            // No frames loaded, ensure needs_redraw is true if we want to display a blank/error state
            state.needs_redraw = true;
        }

        // If needs_redraw is true (either from animation logic or other events like configure), draw the frame.
        if state.needs_redraw {
            if let Err(e) = draw_frame(&mut state, &qh) {
                eprintln!("Error drawing frame: {}", e);
            }
            state.needs_redraw = false; // Reset after drawing
            // If last_frame_time was set by animation logic above, it's already up-to-date for the drawn frame.
            // If it was the very first frame, it was also set above.
        }

        // Clamp sleep duration to a minimum to prevent busy waiting.
        // No upper clamp for now, to respect potentially long GIF delays.
        calculated_sleep_duration = calculated_sleep_duration.max(Duration::from_millis(10));
        std::thread::sleep(calculated_sleep_duration);

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

    if state.gif_frames_rgba.is_empty() {
        return Err("No GIF frames to draw".to_string());
    }
    if state.current_frame_index >= state.gif_frames_rgba.len() {
         return Err(format!("current_frame_index {} out of bounds for gif_frames_rgba (len {})", state.current_frame_index, state.gif_frames_rgba.len()));
    }


    let frame_rgba_data = &state.gif_frames_rgba[state.current_frame_index];
    if frame_rgba_data.is_empty() {
        // This could happen if a frame had no palette and we skipped it.
        // Optionally, draw a placeholder or just skip drawing this frame.
        println!("Skipping draw for empty frame_rgba_data at index {}", state.current_frame_index);
        return Ok(()); // Or return an error/specific state
    }

    let width = state.gif_width as i32;
    let height = state.gif_height as i32;
    let stride = width * 4; // 4 bytes per pixel (RGBA)
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
