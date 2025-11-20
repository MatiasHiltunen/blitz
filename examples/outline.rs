// background: rgb(2,0,36);
// background: linear-gradient(90deg, rgba(2,0,36,1) 0%, rgba(9,9,121,1) 35%, rgba(0,212,255,1) 100%);

use dioxus::prelude::*;

fn main() {
    mini_dxn::launch(app);
}

fn app() -> Element {
    let clip_styles = [
        "clip-path: circle(40%);",
        "clip-path: circle(30% at 50% 60%); shape-margin: 18px;",
        "clip-path: ellipse(430px 440px at 40% 10%);",
        "clip-path: inset(12% 18% round 30px);",
        "clip-path: polygon(50% 0, 100% 50%, 50% 100%, 0 50%);",
        "clip-path: path(\"M0.5,1 C0.5,1,0,0.7,0,0.3 A0.25,0.25,1,1,1,0.5,0.3 A0.25,0.25,1,1,1,1,0.3 C1,0.7,0.5,1,0.5,1 Z\");",
        "clip-path: rect(5px 5px 160px 145px round 20%);",
        "clip-path: url(#shared_polygon_clip);",
    ];

    rsx! {
        style { {CSS} }
        div {
            id: "shared_polygon_clip",
            class: "clip-template",
        }
        div {
          class: "example",
          div {
            class: "imagecontainer",
            "no clip path"
            img {
              class: "image",
              src: "https://images.dog.ceo/breeds/pitbull/dog-3981540_1280.jpg"
            }
          }

          for style in clip_styles {
            div  {
              class: "imagecontainer",
              {style}
              img {
                class: "image",
                style: "{style}",
                src: "https://images.dog.ceo/breeds/pitbull/dog-3981540_1280.jpg"
              }
            }
          }
        }
    }
}

const CSS: &str = r#"
.example {
  display: flex;
  flex-direction: row;
  flex-wrap: wrap;
}
.clip-template {
  position: absolute;
  width: 300px;
  height: 300px;
  left: -9999px;
  top: -9999px;
  clip-path: polygon(0 0, 75% 10%, 100% 50%, 75% 90%, 0 100%);
  pointer-events: none;
}
.image {
  border: solid 1px green;
  width: 300px;
  height: 300px;
}
.imagecontainer {
  border: solid 1px red;
  display: flex;
  flex-direction: column;
  align-items: center;
}
"#;
