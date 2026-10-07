#![allow(dead_code)]
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;
use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

pub struct Guard(pub Child);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Drain both pipes concurrently; a wedged worker cannot hang the test runner.
pub fn run(mut command: Command, input: Vec<u8>) -> Vec<u8> {
    let mut child = Guard(
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut stdin = child.0.stdin.take().unwrap();
    let mut stdout = child.0.stdout.take().unwrap();
    let mut stderr = child.0.stderr.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let reader = std::thread::spawn(move || {
        let mut data = Vec::new();
        stdout.read_to_end(&mut data).unwrap();
        tx.send(data).ok();
    });
    let errors = std::thread::spawn(move || {
        let mut data = String::new();
        stderr.read_to_string(&mut data).unwrap();
        data
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let output = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("worker timed out");
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "worker exit timed out");
        std::thread::sleep(Duration::from_millis(10));
    };
    let stderr = errors.join().unwrap();
    assert!(status.success(), "{stderr}");
    writer.join().unwrap().unwrap();
    reader.join().unwrap();
    output
}

pub fn responses(command: Command, requests: &[Value]) -> Vec<Value> {
    let input = requests
        .iter()
        .map(|r| format!("{r}\n"))
        .collect::<String>();
    let output = run(command, input.into_bytes());
    let responses: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(responses.len(), requests.len());
    for (req, response) in requests.iter().zip(&responses) {
        assert_eq!(req["key"], response["key"]);
    }
    responses
}

/// Decode actual pixels, not just the PNG header, and reject blank renders.
pub fn check_png(req: &Value, response: &Value) {
    assert!(
        response.get("error").is_none_or(Value::is_null),
        "{}: {response}",
        req["formula"]["latex"]
    );
    let bytes = STANDARD.decode(response["png"].as_str().unwrap()).unwrap();
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    let columns = response["columns"].as_u64().unwrap();
    assert_eq!(
        info.width as u64,
        columns * req["cell_width"].as_u64().unwrap()
    );
    assert_eq!(
        info.height as u64,
        req["formula"]["rows"].as_u64().unwrap() * req["cell_height"].as_u64().unwrap()
    );
    assert!(columns > 0 && columns <= req["formula"]["cols"].as_u64().unwrap());
    if req["formula"]["display"] == true {
        assert_eq!(response["columns"], req["formula"]["cols"]);
    }
    let bg = req["formula"]["bg"]
        .as_str()
        .unwrap()
        .trim_start_matches('#');
    let bg: Vec<u8> = (0..6)
        .step_by(2)
        .map(|i| u8::from_str_radix(&bg[i..i + 2], 16).unwrap())
        .collect();
    let channels = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        other => panic!("unexpected PNG color type {other:?}"),
    };
    assert!(
        pixels[..info.buffer_size()]
            .chunks_exact(channels)
            .any(|p| p[..3] != bg && (channels == 3 || p[3] > 0)),
        "blank image: {}",
        req["formula"]["latex"]
    );
    // Optional lossless artifacts for visual inspection, using request keys as indices.
    if let Some(dir) = std::env::var_os("TERMITEX_TEST_ARTIFACTS") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let key = req["key"].as_str().unwrap();
        assert!(key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
        std::fs::write(
            dir.join(format!("{key}.png")),
            STANDARD.decode(response["png"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.join(format!("{key}.json")),
            serde_json::to_vec_pretty(req).unwrap(),
        )
        .unwrap();
    }
}
