//! Interactive clip-path playground with convenient sliders for desktop use.
//! Shapes can be tweaked with sliders; the resulting `clip-path` is also used
//! for a `shape-outside` text wrap demo.

use dioxus::prelude::*;

fn main() {
    mini_dxn::launch(app);
}

const PRESETS: &[(&str, &str)] = &[
    ("Circle (40%)", "circle(40% at 50% 50%)"),
    ("Ellipse", "ellipse(50% 30% at 50% 50%)"),
    ("Inset", "inset(12% 18% round 20px)"),
    ("Diamond", "polygon(50% 0, 100% 50%, 50% 100%, 0 50%)"),
    (
        "Heart",
        r#"path("M0.5 0.9 C0.5 0.9 0.05 0.6 0.05 0.3 C0.05 0.1 0.2 0 0.35 0 C0.45 0 0.55 0.08 0.5 0.2 C0.45 0.08 0.55 0 0.65 0 C0.8 0 0.95 0.1 0.95 0.3 C0.95 0.6 0.5 0.9 0.5 0.9 Z")"#,
    ),
];

const SHAPE_MARGIN_PRESETS: &[(&str, &str)] = &[
    ("shape-margin: 0;", "0px"),
    ("shape-margin: 16px;", "16px"),
    ("shape-margin: 1em;", "1em"),
    ("shape-margin: 5%;", "5%"),
];

fn app() -> Element {
    let mut shape_kind = use_signal(|| String::from("circle"));
    let mut radius_a = use_signal(|| 40.0f32);
    let mut radius_b = use_signal(|| 60.0f32);
    let mut pos_x = use_signal(|| 50.0f32);
    let mut pos_y = use_signal(|| 50.0f32);
    let mut inset_top = use_signal(|| 10.0f32);
    let mut inset_right = use_signal(|| 10.0f32);
    let mut inset_bottom = use_signal(|| 10.0f32);
    let mut inset_left = use_signal(|| 10.0f32);

    let mut clip_path_value = use_signal(|| String::from("circle(40% at 50% 50%)"));
    let mut shape_margin_value = use_signal(|| String::from("12px"));
    let mut image_url =
        use_signal(|| String::from("https://images.dog.ceo/breeds/pitbull/dog-3981540_1280.jpg"));

    // Keep the raw clip-path in sync with builder changes.
    use_effect({
        to_owned![
            shape_kind,
            radius_a,
            radius_b,
            pos_x,
            pos_y,
            inset_top,
            inset_right,
            inset_bottom,
            inset_left,
            clip_path_value
        ];
        move || {
            let built = match shape_kind().as_str() {
                "circle" => format!(
                    "circle({:.0}% at {:.0}% {:.0}%)",
                    radius_a(),
                    pos_x(),
                    pos_y()
                ),
                "ellipse" => format!(
                    "ellipse({:.0}% {:.0}% at {:.0}% {:.0}%)",
                    radius_a(),
                    radius_b(),
                    pos_x(),
                    pos_y()
                ),
                "inset" => format!(
                    "inset({:.0}% {:.0}% {:.0}% {:.0}%)",
                    inset_top(),
                    inset_right(),
                    inset_bottom(),
                    inset_left()
                ),
                "polygon" => String::from("polygon(50% 0, 100% 50%, 50% 100%, 0 50%)"),
                _ => String::from("circle(40% at 50% 50%)"),
            };
            clip_path_value.set(built);
        }
    });

    rsx! {
        style { {CSS} }
        div { class: "shell",
            div { class: "controls",
                h1 { "Clip-Path Playground" }
                h2 { "Shape builder" }
                label { "Shape" }
                select {
                    value: shape_kind(),
                    onchange: move |e| *shape_kind.write() = e.value(),
                    option { value: "circle", "circle" }
                    option { value: "ellipse", "ellipse" }
                    option { value: "inset", "inset" }
                    option { value: "polygon", "polygon (diamond)" }
                }

                div { class: "slider",
                    label { "Radius A ({radius_a():.0}% )" }
                    input {
                        r#type: "range", min: "5", max: "90", step: "1",
                        value: format!("{:.0}", radius_a()),
                        oninput: move |e| *radius_a.write() = e.value().parse().unwrap_or(40.0),
                    }
                }
                { (shape_kind() == "ellipse").then(|| rsx!{
                    div { class: "slider",
                        label { "Radius B ({radius_b():.0}% )" }
                        input {
                            r#type: "range", min: "5", max: "90", step: "1",
                            value: format!("{:.0}", radius_b()),
                            oninput: move |e| *radius_b.write() = e.value().parse().unwrap_or(60.0),
                        }
                    }
                }) }

                { matches!(shape_kind().as_str(), "circle" | "ellipse").then(|| rsx!{
                    div { class: "split",
                        div { class: "slider",
                            label { "Pos X ({pos_x():.0}% )" }
                            input {
                                r#type: "range", min: "0", max: "100", step: "1",
                                value: format!("{:.0}", pos_x()),
                                oninput: move |e| *pos_x.write() = e.value().parse().unwrap_or(50.0),
                            }
                        }
                        div { class: "slider",
                            label { "Pos Y ({pos_y():.0}% )" }
                            input {
                                r#type: "range", min: "0", max: "100", step: "1",
                                value: format!("{:.0}", pos_y()),
                                oninput: move |e| *pos_y.write() = e.value().parse().unwrap_or(50.0),
                            }
                        }
                    }
                }) }

                { (shape_kind() == "inset").then(|| rsx!{
                    div { class: "split inset-grid",
                        div { class: "slider",
                            label { "Top ({inset_top():.0}% )" }
                            input {
                                r#type: "range", min: "0", max: "40", step: "1",
                                value: format!("{:.0}", inset_top()),
                                oninput: move |e| *inset_top.write() = e.value().parse().unwrap_or(10.0),
                            }
                        }
                        div { class: "slider",
                            label { "Right ({inset_right():.0}% )" }
                            input {
                                r#type: "range", min: "0", max: "40", step: "1",
                                value: format!("{:.0}", inset_right()),
                                oninput: move |e| *inset_right.write() = e.value().parse().unwrap_or(10.0),
                            }
                        }
                        div { class: "slider",
                            label { "Bottom ({inset_bottom():.0}% )" }
                            input {
                                r#type: "range", min: "0", max: "40", step: "1",
                                value: format!("{:.0}", inset_bottom()),
                                oninput: move |e| *inset_bottom.write() = e.value().parse().unwrap_or(10.0),
                            }
                        }
                        div { class: "slider",
                            label { "Left ({inset_left():.0}% )" }
                            input {
                                r#type: "range", min: "0", max: "40", step: "1",
                                value: format!("{:.0}", inset_left()),
                                oninput: move |e| *inset_left.write() = e.value().parse().unwrap_or(10.0),
                            }
                        }
                    }
                }) }

                h2 { "Shape margin" }
                div { class: "slider",
                    label { "shape-margin ({shape_margin_value()})" }
                    input {
                        r#type: "range", min: "0", max: "64", step: "1",
                        value: shape_margin_value().replace("px", ""),
                        oninput: move |e| {
                            let px = e.value().parse::<i32>().unwrap_or(12);
                            *shape_margin_value.write() = format!("{px}px");
                        },
                    }
                }

                h2 { "Image URL" }
                input {
                    class: "text",
                    r#type: "url",
                    value: image_url(),
                    oninput: move |e| *image_url.write() = e.value(),
                }

                h2 { "Raw clip-path" }
                textarea {
                    class: "text",
                    rows: "3",
                    value: clip_path_value(),
                    oninput: move |e| *clip_path_value.write() = e.value(),
                }

                h2 { "Presets" }
                div { class: "preset-buttons",
                    for (label, value) in PRESETS.iter() {
                        button {
                            class: "preset-button",
                            onclick: move |_| *clip_path_value.write() = value.to_string(),
                            "{label}"
                        }
                    }
                }
            }

            div { class: "preview",
                div { class: "preview-grid",
                    div { class: "card",
                        h3 { "clip-path only" }
                        div { class: "preview-box",
                            img {
                                class: "preview-image",
                                style: "clip-path: {clip_path_value()}; shape-margin: 0px;",
                                src: "{image_url()}",
                                alt: "Preview without shape-margin"
                            }
                        }
                    }
                    div { class: "card",
                        h3 { "with shape-margin" }
                        div { class: "preview-box",
                            img {
                                class: "preview-image",
                                style: "clip-path: {clip_path_value()}; shape-margin: {shape_margin_value()};",
                                src: "{image_url()}",
                                alt: "Preview with shape-margin applied"
                            }
                        }
                    }
                }

                div { class: "shape-margin-demo",
                    div { class: "shape-margin-header",
                        h3 { "CSS Demo: shape-margin" }
                        button {
                            class: "reset-button",
                            onclick: move |_| {
                                *shape_kind.write() = String::from("circle");
                                *radius_a.write() = 40.0;
                                *radius_b.write() = 60.0;
                                *pos_x.write() = 50.0;
                                *pos_y.write() = 50.0;
                                *shape_margin_value.write() = String::from("12px");
                            },
                            "Reset"
                        }
                    }
                    div { class: "shape-margin-grid",
                        div { class: "shape-margin-sidebar",
                            for (label, value) in SHAPE_MARGIN_PRESETS.iter() {
                                button {
                                    class: format!(
                                        "shape-margin-option {}",
                                        if shape_margin_value() == *value { "active" } else { "" }
                                    ),
                                    onclick: move |_| *shape_margin_value.write() = value.to_string(),
                                    "{label}"
                                }
                            }
                        }
                        div { class: "shape-margin-stage",
                            div {
                                class: "float-shape",
                                style: "clip-path: {clip_path_value()}; shape-outside: {clip_path_value()}; shape-margin: {shape_margin_value()};",
                                img { class: "float-image", src: "{image_url()}", alt: "Floating shape" }
                            }
                            p { class: "demo-copy",
                                "Frenchman belongs to a small set of Parisian sportsmen, who have taken up \"ballooning\" as a pastime. After having exhausted all the sensations that are to be found in ordinary sports, even those of \"automobiling\" at a breakneck speed, the members now seek in the air the nerve-racking excitement that they have ceased to find on earth. Adjust the values to see the margin expand or contract the wrap."
                            }
                        }
                    }
                }

                div { class: "css-output",
                    h3 { "Applied CSS" }
                    code {
                        class: "css-code",
                        "clip-path: {clip_path_value()};\nshape-margin: {shape_margin_value()};"
                    }
                }
            }
        }
    }
}

const CSS: &str = r#"
body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
}

.shell {
  padding: 22px;
  max-width: 1400px;
  margin: 0 auto 40px auto;
  display: grid;
  grid-template-columns: 360px 1fr;
  gap: 20px;
  align-items: start;
}

.controls {
  background: #0f172a;
  color: #e2e8f0;
  border: 1px solid #1e293b;
  border-radius: 12px;
  padding: 18px;
  box-shadow: 0 10px 30px rgba(0,0,0,0.25);
}

.controls h1 {
  margin: 0 0 12px 0;
  font-size: 1.4rem;
}

.controls h2 {
  margin: 18px 0 8px 0;
  font-size: 0.98rem;
  color: #cbd5e1;
}

.slider,
.split {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.split {
  grid-template-columns: 1fr 1fr;
}

.split {
  display: grid;
}

.inset-grid {
  grid-template-columns: repeat(2, 1fr);
  gap: 10px;
}

label {
  font-size: 0.9rem;
}

select,
input[type="range"],
.text {
  width: 100%;
  border-radius: 8px;
  border: 1px solid #1e293b;
  background: #0b1222;
  color: #e2e8f0;
  padding: 8px 10px;
  font-size: 0.95rem;
}

input[type="range"] {
  padding: 0;
  height: 6px;
}

.text {
  font-family: 'Courier New', monospace;
}

.preset-buttons {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  gap: 8px;
}

.preset-button {
  padding: 8px 12px;
  background: #1e293b;
  color: #e2e8f0;
  border: 1px solid #1e293b;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.2s, border-color 0.2s;
}

.preset-button:hover {
  background: #334155;
  border-color: #475569;
}

.preview {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.preview-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
  gap: 14px;
}

.card {
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  padding: 12px;
  box-shadow: 0 6px 18px rgba(0,0,0,0.06);
}

.card h3 {
  margin: 0 0 8px 0;
  font-size: 1rem;
  color: #0f172a;
}

.preview-box {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 300px;
  background: #eff4ff;
  border: 1px dashed #cbd5e1;
  border-radius: 10px;
  padding: 12px;
}

.preview-image {
  width: 260px;
  height: 260px;
  object-fit: cover;
  border-radius: 8px;
  border: 2px solid #3b82f6;
  background: white;
}

.shape-margin-demo {
  background: #0b1222;
  border: 1px solid #1e293b;
  border-radius: 10px;
  color: #e2e8f0;
  box-shadow: 0 8px 30px rgba(0,0,0,0.25);
}

.shape-margin-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 14px 0 14px;
}

.shape-margin-header h3 {
  margin: 0;
  font-size: 1rem;
}

.reset-button {
  background: #1e293b;
  border: 1px solid #334155;
  color: #e2e8f0;
  padding: 6px 10px;
  border-radius: 8px;
  cursor: pointer;
}

.shape-margin-grid {
  display: grid;
  grid-template-columns: 220px 1fr;
  gap: 12px;
  padding: 12px;
}

.shape-margin-sidebar {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.shape-margin-option {
  background: #11192b;
  color: #b5c6dc;
  border: 1px solid #1f2735;
  border-radius: 8px;
  padding: 10px;
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
  background: linear-gradient(135deg, #1c2533, #0f172a);
  color: #e0ecff;
}

.shape-margin-stage {
  background: #0c1016;
  border: 1px solid #1f2630;
  border-radius: 8px;
  padding: 14px;
  min-height: 260px;
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
  line-height: 1.55;
  color: #d1d9e6;
}

.shape-margin-stage::after {
  content: "";
  display: block;
  clear: both;
}

.css-output {
  padding: 12px;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
}

.css-code {
  display: block;
  font-family: 'Courier New', monospace;
  font-size: 14px;
  color: #0f172a;
  white-space: pre-wrap;
  word-break: break-all;
}

@media (max-width: 1100px) {
  .shell {
    grid-template-columns: 1fr;
  }

  .shape-margin-grid {
    grid-template-columns: 1fr;
  }
}
"#;
