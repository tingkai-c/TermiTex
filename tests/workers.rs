mod support;
use serde_json::{Value, json};
use std::process::Command;

#[test]
fn native_rendering_and_error_recovery_without_node() {
    let fixtures: Vec<Value> = serde_json::from_str(include_str!("fixtures/render.json")).unwrap();
    let mut requests: Vec<Value> = fixtures.iter().map(|f| f["request"].clone()).collect();
    let latex: Vec<String> = serde_json::from_str(include_str!("fixtures/native.json")).unwrap();
    for latex in latex {
        let mut r = requests[0].clone();
        r["formula"]["latex"] = json!(latex);
        r["formula"]["display"] = json!(true);
        r["formula"]["rows"] = json!(4);
        r["formula"]["cols"] = json!(100);
        requests.push(r);
    }
    for (cw, ch) in [(8, 17), (12, 24), (20, 42)] {
        let mut r = requests[2].clone();
        r["cell_width"] = json!(cw);
        r["cell_height"] = json!(ch);
        requests.push(r);
    }
    for (i, r) in requests.iter_mut().enumerate() {
        r["key"] = json!(format!("native-{i}"));
        r["formula"]["row"] = json!(0);
        r["formula"]["col"] = json!(0);
    }
    let valid = requests.len();
    assert_eq!(valid, 31);
    for (i, latex) in [
        r"\notarealcommand".into(),
        r"\frac{".into(),
        format!("{}x{}", "{".repeat(40), "}".repeat(40)),
        "x".repeat(20001),
        r"\dv{f}{x}".into(),
    ]
    .iter()
    .enumerate()
    {
        let mut r = requests[0].clone();
        r["key"] = json!(format!("invalid-{i}"));
        r["formula"]["latex"] = json!(latex);
        requests.push(r);
    }
    let mut recovery = requests[0].clone();
    recovery["key"] = json!("recovery");
    requests.push(recovery);
    let mut command = Command::new(env!("CARGO_BIN_EXE_termitex"));
    command
        .arg("--internal-ratex-worker")
        .env("PATH", "/usr/bin:/bin");
    let responses = support::responses(command, &requests);
    for (r, response) in requests[..valid].iter().zip(&responses) {
        support::check_png(r, response);
    }
    for response in &responses[valid..valid + 5] {
        assert!(
            response["error"].as_str().is_some_and(|s| !s.is_empty()),
            "{response}"
        );
    }
    support::check_png(requests.last().unwrap(), responses.last().unwrap());
}

#[test]
#[ignore = "requires Node.js and npm ci --ignore-scripts"]
fn mathjax_worker_png_dimensions_and_compaction() {
    let requests: Vec<Value> = [("inline", "J", 1, 5, false), ("display", r"\displaystyle\frac{1}{2}", 3, 80, true), ("curl", r"\nabla\times", 1, 20, false)].into_iter().map(|(key, latex, rows, cols, display)| json!({"key":key,"cell_width":16,"cell_height":34,"formula":{"latex":latex,"row":0,"col":0,"rows":rows,"cols":cols,"display":display,"fg":"#ffffff","bg":"#282c34"}})).collect();
    let mut command = Command::new("node");
    command.arg(concat!(env!("CARGO_MANIFEST_DIR"), "/worker/render.mjs"));
    let responses = support::responses(command, &requests);
    for (r, response) in requests.iter().zip(&responses) {
        support::check_png(r, response);
        if r["formula"]["display"] == false {
            assert!(response["columns"].as_u64() < r["formula"]["cols"].as_u64());
        }
    }
}

fn compatibility_renders(command: Command) {
    let requests: Vec<Value> = [("compat-inline", false, 1), ("compat-display", true, 3)].into_iter().map(|(key,display,rows)| json!({"key":key,"compatibility":true,"cell_width":16,"cell_height":34,"formula":{"latex":"x+x","row":0,"col":0,"rows":rows,"cols":30,"display":display,"fg":"#ffffff","bg":"#282c34"}})).collect();
    let responses = support::responses(command, &requests);
    for (req, response) in requests.iter().zip(&responses) {
        support::check_png(req, response);
    }
}
#[test]
fn native_compatibility_centers_without_compacting() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_termitex"));
    command
        .arg("--internal-ratex-worker")
        .env("PATH", "/usr/bin:/bin");
    compatibility_renders(command);
}
#[test]
#[ignore = "requires Node.js and npm ci --ignore-scripts"]
fn mathjax_compatibility_centers_without_compacting() {
    let mut command = Command::new("node");
    command.arg(concat!(env!("CARGO_MANIFEST_DIR"), "/worker/render.mjs"));
    compatibility_renders(command);
}
