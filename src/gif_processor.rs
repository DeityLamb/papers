use std::fs::File;
use std::path::PathBuf;
use gif::{DecodeOptions}; // Removed unused Frame as GifNativeFrame
// use std::io::Read; // Removed unused Read

#[derive(Debug)]
pub struct GifData {
    pub width: u16,
    pub height: u16,
    pub frames_bgra: Vec<Vec<u8>>, // Storing BGRA data
    pub frame_delays: Vec<u16>,   // Delays in 1/100ths of a second
}

impl GifData {
    pub fn new(path: &PathBuf) -> Result<Self, String> {
        let file = File::open(path)
            .map_err(|e| format!("Failed to open GIF file '{}': {}", path.display(), e))?;

        let options = DecodeOptions::new();
        // gif 0.12.0 decoder.read_info takes R: Read. File itself implements Read.
        let mut decoder = options.read_info(file)
            .map_err(|e| format!("Failed to read GIF info from '{}': {}", path.display(), e))?;

        let width = decoder.width();
        let height = decoder.height();
        let global_palette = decoder.global_palette().unwrap_or_default().to_vec();

        let mut frames_bgra = Vec::new();
        let mut frame_delays = Vec::new();

        while let Some(frame) = decoder.read_next_frame()
            .map_err(|e| format!("Failed to read next GIF frame: {}", e))?
        {
            let frame_info = frame.to_owned(); // Get owned Frame info
            frame_delays.push(frame_info.delay);

            let buffer_capacity = (frame_info.width as usize) * (frame_info.height as usize) * 4;
            let mut bgra_buffer = Vec::with_capacity(buffer_capacity);

            // Clear buffer with opaque black (BGRA: 0,0,0,255)
            bgra_buffer.resize(buffer_capacity, 0); // Initialize with 0s
            for chunk in bgra_buffer.chunks_mut(4) {
                chunk[0] = 0;   // B
                chunk[1] = 0;   // G
                chunk[2] = 0;   // R
                chunk[3] = 255; // A (opaque)
            }


            let current_palette = frame_info.palette.as_ref().unwrap_or(&global_palette);

            if current_palette.is_empty() && !frame_info.buffer.is_empty() {
                // If no palette but there's data, this is problematic.
                // For now, we've filled with black. Could log a warning.
                eprintln!("Warning: Frame in {} has data but no palette (global or local). Rendered as black.", path.display());
            }

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
                    // Index out of bounds for palette, use default black (already set by clear)
                    r = 0; g = 0; b = 0;
                }

                let base = pixel_idx * 4;
                if base + 3 < bgra_buffer.len() { // Check bounds before writing
                    bgra_buffer[base] = b;
                    bgra_buffer[base + 1] = g;
                    bgra_buffer[base + 2] = r;
                    bgra_buffer[base + 3] = a;
                }
                pixel_idx += 1;
            }
            frames_bgra.push(bgra_buffer);
        }

        if frames_bgra.is_empty() {
            return Err(format!("No frames found or processed in GIF '{}'", path.display()));
        }

        Ok(Self {
            width,
            height,
            frames_bgra,
            frame_delays,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::fs;

    // Helper function to save a BGRA frame as a PPM P6 file
    fn save_bgra_as_ppm(width: u16, height: u16, bgra_data: &[u8], filename: &str) -> std::io::Result<()> {
        let mut file = File::create(filename)?;
        writeln!(file, "P6")?;
        writeln!(file, "{} {}", width, height)?;
        writeln!(file, "255")?; // Max color value

        let mut rgb_data = Vec::with_capacity(width as usize * height as usize * 3);
        for bgra_chunk in bgra_data.chunks_exact(4) {
            let b = bgra_chunk[0];
            let g = bgra_chunk[1];
            let r = bgra_chunk[2];
            // PPM P6 expects RGB order
            rgb_data.push(r);
            rgb_data.push(g);
            rgb_data.push(b);
        }
        file.write_all(&rgb_data)?;
        Ok(())
    }

    #[test]
    fn test_load_holo_gif_metadata() {
        // Create a dummy holo.gif for testing metadata.
        // The actual content will be replaced by the user.
        // For this test to pass without the real holo.gif, we use known dummy values.
        let dummy_gif_path = PathBuf::from("test_dummy_holo.gif");
        // Minimal valid 1x1 transparent GIF (R0lGODlhAQABAAAAACH5BAEKAAEALAAAAAABAAEAAAICTAEAOw==)
        // For simplicity, let's use a text file and expect it to fail parsing in a specific way,
        // or create a known small GIF if possible via a helper.
        // For now, this test will likely fail if holo.gif is just a text placeholder.
        // The user is expected to provide the actual holo.gif.
        // Let's assume holo.gif is present for the test.

        // Create a dummy holo.gif if it doesn't exist for basic test structure
        // This is just so `cargo test` doesn't fail immediately on file not found.
        // The user must provide the real holo.gif for meaningful tests.
        let test_gif_path = PathBuf::from("holo.gif");
        if !test_gif_path.exists() {
            let _ = File::create(&test_gif_path).map_err(|e| eprintln!("Failed to create dummy holo.gif for test: {}", e));
            // Consider writing a minimal valid GIF here if possible, otherwise tests might be limited.
        }


        match GifData::new(&test_gif_path) {
            Ok(gif_data) => {
                // These assertions will depend on the actual holo.gif's properties.
                // User will need to adjust these if holo.gif changes or has different props.
                // For now, using placeholder values that are likely to be true for many GIFs.
                assert!(gif_data.width > 0, "GIF width should be greater than 0");
                assert!(gif_data.height > 0, "GIF height should be greater than 0");
                assert!(!gif_data.frames_bgra.is_empty(), "Should load at least one frame");
                assert_eq!(gif_data.frames_bgra.len(), gif_data.frame_delays.len(), "Frame data and delay counts should match");

                let expected_pixel_data_len = (gif_data.width as usize) * (gif_data.height as usize) * 4;
                for (i, frame_data) in gif_data.frames_bgra.iter().enumerate() {
                    assert_eq!(frame_data.len(), expected_pixel_data_len, "Frame {} data length is incorrect", i);
                }
                 eprintln!("Successfully loaded holo.gif metadata: {}x{}, {} frames.", gif_data.width, gif_data.height, gif_data.frames_bgra.len());

            }
            Err(e) => {
                // If holo.gif is the placeholder, this error is expected.
                // For a real test, this should be assert!(false, "Failed to load holo.gif: {}", e);
                eprintln!("Note: Failed to load holo.gif for metadata test (this is expected if using placeholder file): {}", e);
                // To make this test pass with a placeholder, we can assert that the error occurs.
                // For now, let's assume the user provides holo.gif and it should load.
                 assert!(false, "Failed to load holo.gif, ensure it's present and valid: {}", e);
            }
        }
    }

    #[test]
    fn test_save_first_few_frames_as_ppm() {
        let test_gif_path = PathBuf::from("holo.gif");
        // Ensure the test output directory exists
        let output_dir = PathBuf::from("test_frames_output");
        if !output_dir.exists() {
            fs::create_dir_all(&output_dir).expect("Failed to create test output directory");
        }

        match GifData::new(&test_gif_path) {
            Ok(gif_data) => {
                let num_frames_to_save = std::cmp::min(gif_data.frames_bgra.len(), 3); // Save up to 3 frames
                for i in 0..num_frames_to_save {
                    let filename = output_dir.join(format!("frame_{:03}.ppm", i));
                    save_bgra_as_ppm(
                        gif_data.width,
                        gif_data.height,
                        &gif_data.frames_bgra[i],
                        filename.to_str().unwrap()
                    ).expect(&format!("Failed to save frame {} to PPM", i));
                    eprintln!("Saved frame {} to {:?}", i, filename);
                }
                // This test primarily checks if saving runs without error. Visual inspection of PPMs is manual.
            }
            Err(e) => {
                 assert!(false, "Failed to load holo.gif for PPM saving test, ensure it's present and valid: {}", e);
            }
        }
    }
}
