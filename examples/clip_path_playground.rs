//! Interactive clip-path playground
//!
//! This example allows users to experiment with different clip-path values
//! and see the results in real-time.

use dioxus::prelude::*;

fn main() {
    mini_dxn::launch(app);
}

const PRESETS: &[(&str, &str)] = &[
    ("Reset", "circle(40%)"),
    ("Circle (40%)", "circle(40%)"),
    ("Circle (30% at 50% 60%)", "circle(30% at 50% 60%)"),
    (
        "Ellipse (130px 140px at 10% 20%)",
        "ellipse(130px 140px at 10% 20%)",
    ),
    ("Ellipse (50% 30%)", "ellipse(50% 30%)"),
    (
        "Ellipse (40% 60% at 50% 50%)",
        "ellipse(40% 60% at 50% 50%)",
    ),
    ("Inset", "inset(12% 18% round 30px)"),
    (
        "Polygon (Diamond)",
        "polygon(50% 0, 100% 50%, 50% 100%, 0 50%)",
    ),
    ("Polygon (Triangle)", "polygon(50% 0, 100% 100%, 0 100%)"),
    (
        "Path (Heart)",
        // Normalized 0..1 heart that scales with the reference box
        r#"path("M0.5 0.9 C0.5 0.9 0.05 0.6 0.05 0.3 C0.05 0.1 0.2 0 0.35 0 C0.45 0 0.55 0.08 0.5 0.2 C0.45 0.08 0.55 0 0.65 0 C0.8 0 0.95 0.1 0.95 0.3 C0.95 0.6 0.5 0.9 0.5 0.9 Z")"#,
    ),
    (
        "Path (Arc + Lines)",
        r#"path("M0 1 L0 0.4 A0.3 0.3 0 0 1 0.8 0.4 L1 1 Z")"#,
    ),
    ("Rect()", "rect(5px 145px 160px 5px round 20%)"),
    ("XYWH()", "xywh(0 5px 100% 75% round 15% 0)"),
    (
        "Geometry Box: padding-box circle",
        "padding-box circle(50px at 0 100px)",
    ),
    ("Geometry Box: border-box", "border-box"),
    ("Geometry Box: margin-box", "margin-box"),
    ("SVG url sample", r#"url("resources.svg#c1")"#),
    ("None", "none"),
];

const SHAPE_MARGIN_PRESETS: &[(&str, &str)] = &[
    ("shape-margin: 0;", "0px"),
    ("shape-margin: 20px;", "20px"),
    ("shape-margin: 1em;", "1em"),
    ("shape-margin: 5%;", "5%"),
];

fn app() -> Element {
    let mut clip_path_value = use_signal(|| String::from("circle(40%)"));
    let mut shape_margin_value = use_signal(|| String::from("0px"));
    let mut image_url =
        use_signal(|| String::from("https://images.dog.ceo/breeds/pitbull/dog-3981540_1280.jpg"));

    rsx! {
        style { {CSS} }
        div {
            class: "playground-container",
            h1 { "Clip-Path Playground" }
            div {
                class: "controls",
                div {
                    class: "control-group",
                    label {
                        r#for: "clip-path-input",
                        "Clip-Path:"
                    }
                    input {
                        id: "clip-path-input",
                        class: "clip-path-input",
                        r#type: "text",
                        value: clip_path_value(),
                        placeholder: "e.g., circle(40%)",
                        oninput: move |evt| {
                            *clip_path_value.write() = evt.value();
                        },
                    }
                }
                div {
                    class: "control-group",
                    label {
                        r#for: "shape-margin-input",
                        "Shape-Margin:"
                    }
                    input {
                        id: "shape-margin-input",
                        class: "shape-margin-input",
                        r#type: "text",
                        value: shape_margin_value(),
                        placeholder: "e.g., 10px",
                        oninput: move |evt| {
                            *shape_margin_value.write() = evt.value();
                        },
                    }
                }
                div {
                    class: "control-group",
                    label {
                        r#for: "image-url-input",
                        "Image URL:"
                    }
                    input {
                        id: "image-url-input",
                        class: "image-url-input",
                        r#type: "url",
                        value: image_url(),
                        placeholder: "https://example.com/image.jpg",
                        oninput: move |evt| {
                            *image_url.write() = evt.value();
                        },
                    }
                }
            }
            div {
                class: "presets",
                h2 { "Presets:" }
                div {
                    class: "preset-buttons",
                    for (label, value) in PRESETS.iter() {
                        button {
                            class: "preset-button",
                            onclick: move |_| {
                                *clip_path_value.write() = value.to_string();
                            },
                            {*label}
                        }
                    }
                }
            }
            div {
                class: "preview-section",
                h2 { "Preview:" }
                div {
                    class: "comparison-row",
                    div {
                        class: "preview-column",
                        h3 { "No shape-margin" }
                        div {
                            class: "preview-box",
                            img {
                                class: "preview-image",
                                style: "clip-path: {clip_path_value()}; shape-margin: 0px;",
                                src: "{image_url()}",
                                alt: "Preview without shape-margin"
                            }
                        }
                        p { class: "legend", "clip-path only" }
                    }
                    div {
                        class: "preview-column",
                        h3 { "With shape-margin" }
                        div {
                            class: "preview-box",
                            img {
                                class: "preview-image",
                                style: "clip-path: {clip_path_value()}; shape-margin: {shape_margin_value()};",
                                src: "{image_url()}",
                                alt: "Preview with shape-margin applied"
                            }
                        }
                        p { class: "legend", "shape-margin: {shape_margin_value()}" }
                    }
                }
                div {
                    class: "shape-margin-demo",
                    div {
                        class: "shape-margin-header",
                        h3 { "CSS Demo: shape-margin" }
                        button {
                            class: "reset-button",
                            onclick: move |_| {
                                *clip_path_value.write() = String::from("circle(40%)");
                                *shape_margin_value.write() = String::from("0px");
                            },
                            "Reset"
                        }
                    }
                    div { class: "shape-margin-grid",
                        div { class: "shape-margin-sidebar",
                            for (label, value) in SHAPE_MARGIN_PRESETS.iter() {
                                button {
                                    class: {format!(
                                        "shape-margin-option {}",
                                        if shape_margin_value() == *value { "active" } else { "" }
                                    )},
                                    onclick: move |_| {
                                        *shape_margin_value.write() = value.to_string();
                                    },
                                    "{label}"
                                }
                            }
                        }
                        div { class: "shape-margin-stage",
                            div {
                                class: "float-shape dark",
                                style: "clip-path: {clip_path_value()}; shape-outside: {clip_path_value()}; shape-margin: {shape_margin_value()};",
                                img {
                                    class: "float-image",
                                    src: "{image_url()}",
                                    alt: "Floating shape"
                                }
                            }
                            p { class: "demo-copy dark",
                                "Frenchman belongs to a small set of Parisian sportsmen, who have taken up “ballooning” as a pastime. After having exhausted all the sensations that are to be found in ordinary sports, even those of “automobiling” at a breakneck speed, the members now seek in the air the nerve-racking excitement that they have ceased to find on earth. Adjust the values to see the margin expand or contract the wrap."
                            }
                        }
                    }
                }
                div {
                    class: "css-output",
                    h3 { "Applied CSS:" }
                    code {
                        class: "css-code",
                        "clip-path: {clip_path_value()};"
                        if !shape_margin_value().is_empty() && shape_margin_value() != "0px" {
                            "\nshape-margin: {shape_margin_value()};"
                        }
                    }
                }
            }
        }
    }
}

const CSS: &str = r#"
.playground-container {
  padding: 20px;
  max-width: 1200px;
  margin: 0 auto;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
}

h1 {
  color: #333;
  margin-bottom: 30px;
  text-align: center;
}

h2 {
  color: #555;
  margin-top: 30px;
  margin-bottom: 15px;
  font-size: 1.2em;
}

h3 {
  color: #666;
  margin-top: 20px;
  margin-bottom: 10px;
  font-size: 1em;
}

.controls {
  display: flex;
  gap: 20px;
  margin-bottom: 30px;
  flex-wrap: wrap;
}

.control-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
  min-width: 250px;
}

.control-group label {
  font-weight: 600;
  color: #444;
  font-size: 0.9em;
}

.clip-path-input,
.shape-margin-input,
.image-url-input {
  padding: 10px;
  border: 2px solid #ddd;
  border-radius: 4px;
  font-family: 'Courier New', monospace;
  font-size: 14px;
  transition: border-color 0.2s;
}

.clip-path-input:focus,
.shape-margin-input:focus,
.image-url-input:focus {
  outline: none;
  border-color: #4CAF50;
}

.image-url-input {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
}

.presets {
  margin-bottom: 30px;
}

.preset-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.preset-button {
  padding: 8px 16px;
  background-color: #4CAF50;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 14px;
  transition: background-color 0.2s;
}

.preset-button:hover {
  background-color: #45a049;
}

.preset-button:active {
  background-color: #3d8b40;
}

.preview-section {
  margin-top: 40px;
}

.comparison-row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
  gap: 24px;
}

.preview-column {
  background: #fafafa;
  border: 1px solid #e3e3e3;
  border-radius: 8px;
  padding: 12px;
  box-shadow: 0 4px 10px rgba(0,0,0,0.04);
}

.preview-column h3 {
  margin: 0 0 10px 0;
  font-size: 1rem;
  color: #333;
}

.preview-box {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 320px;
  background: linear-gradient(45deg, #f0f0f0 25%, transparent 25%),
              linear-gradient(-45deg, #f0f0f0 25%, transparent 25%),
              linear-gradient(45deg, transparent 75%, #f0f0f0 75%),
              linear-gradient(-45deg, transparent 75%, #f0f0f0 75%);
  background-size: 20px 20px;
  background-position: 0 0, 0 10px, 10px -10px, -10px 0px;
  border: 2px dashed #c7c7c7;
  border-radius: 8px;
  padding: 16px;
}

.preview-image {
  display: block;
  width: 280px;
  height: 280px;
  object-fit: cover;
  border-radius: 6px;
  border: 2px solid #4CAF50;
  background: white;
  transition: all 0.2s ease;
}

.legend {
  margin-top: 8px;
  color: #666;
  font-size: 0.9em;
  text-align: center;
}

.css-output {
  margin-top: 20px;
  padding: 15px;
  background-color: #f5f5f5;
  border-radius: 4px;
  border-left: 4px solid #4CAF50;
}

.css-code {
  display: block;
  font-family: 'Courier New', monospace;
  font-size: 14px;
  color: #333;
  white-space: pre-wrap;
  word-break: break-all;
}

.shape-margin-demo {
  margin-top: 32px;
  background: #101317;
  border: 1px solid #1f2630;
  border-radius: 10px;
  color: #e5ecf5;
  box-shadow: 0 8px 30px rgba(0,0,0,0.25);
}

.shape-margin-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 16px 0 16px;
}

.shape-margin-grid {
  display: grid;
  grid-template-columns: 240px 1fr;
  gap: 16px;
  padding: 12px 16px 16px 16px;
}

.shape-margin-sidebar {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.shape-margin-option {
  background: #161b23;
  color: #b5c6dc;
  border: 1px solid #252f3c;
  border-radius: 8px;
  padding: 12px;
  text-align: left;
  font-family: 'Courier New', monospace;
  cursor: pointer;
  transition: all 0.15s ease;
}

.shape-margin-option:hover {
  border-color: #3b82f6;
  color: #e5ecf5;
}

.shape-margin-option.active {
  border-color: #3b82f6;
  background: linear-gradient(135deg, #1c2533, #111827);
  color: #e0ecff;
}

.shape-margin-stage {
  background: #0c1016;
  border: 1px solid #1f2630;
  border-radius: 8px;
  padding: 16px;
  position: relative;
  min-height: 260px;
  overflow: auto;
}

.float-shape {
  float: left;
  width: 180px;
  height: 180px;
  margin: 0 18px 8px 0;
  border-radius: 50%;
}

.float-image {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: 50%;
}

.demo-copy {
  text-align: left;
  line-height: 1.5;
  color: #d1d9e6;
}

.shape-margin-demo::after,
.shape-margin-stage::after {
  content: "";
  display: block;
  clear: both;
}

@media (max-width: 768px) {
  .controls {
    flex-direction: column;
  }

  .control-group {
    min-width: 100%;
  }

  .preset-buttons {
    justify-content: center;
  }

  .preview-image {
    width: 250px;
    height: 250px;
  }
}
"#;
