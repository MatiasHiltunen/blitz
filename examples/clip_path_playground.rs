//! Interactive clip-path playground
//! 
//! This example allows users to experiment with different clip-path values
//! and see the results in real-time.

use dioxus::prelude::*;

fn main() {
    mini_dxn::launch(app);
}

const PRESETS: &[(&str, &str)] = &[
    ("Circle (40%)", "circle(40%)"),
    ("Circle (30% at 50% 60%)", "circle(30% at 50% 60%)"),
    ("Ellipse (Large)", "ellipse(430px 440px at 40% 10%)"),
    ("Ellipse (50% 30%)", "ellipse(50% 30%)"),
    ("Ellipse (40% 60% at 50% 50%)", "ellipse(40% 60% at 50% 50%)"),
    ("Inset", "inset(12% 18% round 30px)"),
    ("Polygon (Diamond)", "polygon(50% 0, 100% 50%, 50% 100%, 0 50%)"),
    ("Polygon (Triangle)", "polygon(50% 0, 100% 100%, 0 100%)"),
    ("Path (Heart)", r#"path("M0.5,1 C0.5,1,0,0.7,0,0.3 A0.25,0.25,1,1,1,0.5,0.3 A0.25,0.25,1,1,1,1,0.3 C1,0.7,0.5,1,0.5,1 Z")"#),
    ("Rect", "rect(5px 5px 160px 145px round 20%)"),
    ("None", "none"),
];

fn app() -> Element {
    let mut clip_path_value = use_signal(|| String::from("circle(40%)"));
    let mut shape_margin_value = use_signal(|| String::from("0px"));
    let mut image_url = use_signal(|| String::from("https://images.dog.ceo/breeds/pitbull/dog-3981540_1280.jpg"));

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
                    class: "preview-container",
                    div {
                        class: "preview-box",
                        img {
                            class: "preview-image",
                            style: "clip-path: {clip_path_value()}; shape-margin: {shape_margin_value()};",
                            src: "{image_url()}",
                            alt: "Preview image with clip-path applied"
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

.preview-container {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 400px;
  background: linear-gradient(45deg, #f0f0f0 25%, transparent 25%),
              linear-gradient(-45deg, #f0f0f0 25%, transparent 25%),
              linear-gradient(45deg, transparent 75%, #f0f0f0 75%),
              linear-gradient(-45deg, transparent 75%, #f0f0f0 75%);
  background-size: 20px 20px;
  background-position: 0 0, 0 10px, 10px -10px, -10px 0px;
  border: 2px solid #ddd;
  border-radius: 8px;
  padding: 20px;
}

.preview-box {
  display: inline-block;
  border: 2px solid #4CAF50;
  border-radius: 4px;
  padding: 10px;
  background: white;
}

.preview-image {
  display: block;
  width: 300px;
  height: 300px;
  object-fit: cover;
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

